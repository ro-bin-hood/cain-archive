#!/usr/bin/env python3
# ia_downloader.py - Downloader minimale per Internet Archive
import tkinter as tk
from tkinter import ttk, filedialog, messagebox
import threading, queue, urllib.request, urllib.parse, urllib.error, json, os, re, sys, time
from concurrent.futures import ThreadPoolExecutor

UA = {"User-Agent": "ia-mini-downloader/1.0"}
SETTINGS = os.path.join(os.path.expanduser("~"), ".ia_downloader.json")
CHUNK = 256 * 1024
SKIP_SUFFIX = ("_meta.xml", "_files.xml", "_meta.sqlite", "_archive.torrent", "_reviews.xml")
AUTH = {}   # {"user", "cookies": {...}, "s3": {"access", "secret"}} dopo il login

def headers(extra=None):
    h = dict(UA)
    if AUTH:
        h["Cookie"] = "; ".join(f"{k}={v}" for k, v in AUTH["cookies"].items())
        h["Authorization"] = f"LOW {AUTH['s3']['access']}:{AUTH['s3']['secret']}"
    if extra: h.update(extra)
    return h

def login(email, password):
    data = urllib.parse.urlencode({"email": email, "password": password}).encode()
    req = urllib.request.Request("https://archive.org/services/xauthn/?op=login", data=data, headers=UA)
    try:
        with urllib.request.urlopen(req, timeout=30) as r: j = json.load(r)
    except urllib.error.HTTPError as e:
        j = json.load(e)
    if not j.get("success"):
        reason = j.get("values", {}).get("reason", "sconosciuto")
        raise RuntimeError({"account_not_found": "Account inesistente",
                            "account_bad_password": "Password errata"}.get(reason, f"Login fallito ({reason})"))
    v = j["values"]
    return {"user": v.get("screenname") or email,
            "cookies": {k: v["cookies"][k] for k in ("logged-in-user", "logged-in-sig")},
            "s3": {"access": v["s3"]["access"], "secret": v["s3"]["secret"]}}

def human(n):
    for u in ("B", "KB", "MB", "GB", "TB"):
        if n < 1024: return f"{n:.1f} {u}"
        n /= 1024
    return f"{n:.1f} PB"

def parse_link(link):
    link = link.strip()
    if not link: return None
    m = re.match(r"https?://(?:www\.)?archive\.org/(details|download)/([^/?#]+)(?:/([^?#]+))?", link)
    if m:
        kind, ident, path = m.groups()
        if kind == "download" and path:
            return ident, urllib.parse.unquote(path)
        return ident, None
    if re.fullmatch(r"[\w.\-]+", link):   # identificatore nudo
        return link, None
    return None

def list_files(ident, originals_only, exts):
    req = urllib.request.Request(f"https://archive.org/metadata/{ident}", headers=headers())
    with urllib.request.urlopen(req, timeout=30) as r:
        meta = json.load(r)
    if not meta.get("files"):
        hint = "" if AUTH else " (prova ad accedere)"
        raise RuntimeError(f"Item '{ident}' vuoto, inesistente o ad accesso ristretto{hint}")
    out = []
    for f in meta["files"]:
        name = f["name"]
        if name.endswith(SKIP_SUFFIX): continue
        if originals_only and f.get("source") != "original": continue
        if exts and not name.lower().endswith(exts): continue
        out.append((name, int(f.get("size") or 0)))
    return out

class App:
    def __init__(self, root):
        self.root = root
        root.title("Internet Archive Downloader")
        root.geometry("950x620")
        icon = os.path.join(getattr(sys, "_MEIPASS", os.path.dirname(os.path.abspath(__file__))), "logo.png")
        if os.path.exists(icon):
            self.icon = tk.PhotoImage(file=icon); root.iconphoto(True, self.icon)
        self.q = queue.Queue()
        self.outdir = tk.StringVar(value=os.path.join(os.path.expanduser("~"), "Downloads", "archive"))
        self.workers = tk.IntVar(value=3)
        self.originals = tk.BooleanVar(value=True)
        self.exts = tk.StringVar(value="")
        self.stop_flag = threading.Event()
        self.jobs = {}

        f = ttk.Frame(root, padding=8); f.pack(fill="both", expand=True)
        ttk.Label(f, text="Link (uno per riga) - pagine /details/, /download/ o identificatori:").pack(anchor="w")
        self.links = tk.Text(f, height=6); self.links.pack(fill="x")

        opt = ttk.Frame(f); opt.pack(fill="x", pady=6)
        ttk.Label(opt, text="Cartella:").pack(side="left")
        ttk.Entry(opt, textvariable=self.outdir, width=45).pack(side="left", padx=4)
        ttk.Button(opt, text="…", width=3, command=self.pick).pack(side="left")
        ttk.Label(opt, text="  Paralleli:").pack(side="left")
        ttk.Spinbox(opt, from_=1, to=8, textvariable=self.workers, width=3).pack(side="left")
        ttk.Checkbutton(opt, text="Solo originali", variable=self.originals).pack(side="left", padx=8)
        ttk.Label(opt, text="Estensioni (es. pdf,mp3):").pack(side="left")
        ttk.Entry(opt, textvariable=self.exts, width=14).pack(side="left", padx=4)

        btn = ttk.Frame(f); btn.pack(fill="x")
        self.start_btn = ttk.Button(btn, text="▶ Avvia", command=self.start); self.start_btn.pack(side="left")
        ttk.Button(btn, text="■ Stop", command=self.stop).pack(side="left", padx=4)
        ttk.Button(btn, text="Pulisci completati", command=self.clear_done).pack(side="left")
        self.login_btn = ttk.Button(btn, command=self.toggle_login); self.login_btn.pack(side="right")
        self.user_lbl = ttk.Label(btn); self.user_lbl.pack(side="right", padx=6)
        self.total_lbl = ttk.Label(btn, text=""); self.total_lbl.pack(side="right", padx=12)

        cols = ("file", "size", "progress", "speed", "status")
        self.tree = ttk.Treeview(f, columns=cols, show="headings")
        for c, w in zip(cols, (430, 90, 90, 90, 200)):
            self.tree.heading(c, text=c.capitalize()); self.tree.column(c, width=w, anchor="w")
        self.tree.pack(fill="both", expand=True, pady=6)
        self.overall = ttk.Progressbar(f, mode="determinate"); self.overall.pack(fill="x")
        self.load_settings()
        self.refresh_login()
        root.protocol("WM_DELETE_WINDOW", self.on_close)
        self.root.after(200, self.poll)

    def refresh_login(self):
        self.user_lbl.config(text=f"Connesso come {AUTH['user']}" if AUTH else "Non connesso")
        self.login_btn.config(text="Esci" if AUTH else "Accedi…")

    def toggle_login(self):
        if AUTH:
            AUTH.clear(); self.save_settings(); self.refresh_login(); return
        dlg = tk.Toplevel(self.root); dlg.title("Accedi a archive.org")
        dlg.resizable(False, False); dlg.transient(self.root); dlg.grab_set()
        fr = ttk.Frame(dlg, padding=12); fr.pack()
        email, pw = tk.StringVar(), tk.StringVar()
        ttk.Label(fr, text="Email:").grid(row=0, column=0, sticky="w")
        e = ttk.Entry(fr, textvariable=email, width=32); e.grid(row=0, column=1, pady=2); e.focus()
        ttk.Label(fr, text="Password:").grid(row=1, column=0, sticky="w")
        ttk.Entry(fr, textvariable=pw, show="•", width=32).grid(row=1, column=1, pady=2)
        msg = ttk.Label(fr, foreground="#b00020"); msg.grid(row=2, column=0, columnspan=2, sticky="w")
        ok = ttk.Button(fr, text="Accedi"); ok.grid(row=3, column=1, sticky="e", pady=(6, 0))

        def done(result):
            if not dlg.winfo_exists(): return
            if isinstance(result, Exception):
                msg.config(text=str(result)); ok.state(["!disabled"]); return
            AUTH.clear(); AUTH.update(result)
            self.save_settings(); self.refresh_login(); dlg.destroy()

        def submit(*_):
            if not email.get() or not pw.get(): return
            ok.state(["disabled"]); msg.config(text="Accesso in corso…", foreground="")
            def work():
                try: r = login(email.get().strip(), pw.get())
                except Exception as ex: r = ex
                self.root.after(0, lambda: (msg.config(foreground="#b00020"), done(r)))
            threading.Thread(target=work, daemon=True).start()
        ok.config(command=submit); dlg.bind("<Return>", submit)

    def load_settings(self):
        try:
            with open(SETTINGS, encoding="utf-8") as fh: s = json.load(fh)
        except (OSError, ValueError):
            return
        if s.get("auth"): AUTH.update(s["auth"])
        self.links.insert("1.0", s.get("links", ""))
        self.outdir.set(s.get("outdir", self.outdir.get()))
        self.workers.set(s.get("workers", self.workers.get()))
        self.originals.set(s.get("originals", self.originals.get()))
        self.exts.set(s.get("exts", self.exts.get()))

    def save_settings(self):
        s = {"links": self.links.get("1.0", "end-1c"), "outdir": self.outdir.get(),
             "workers": self.workers.get(), "originals": self.originals.get(), "exts": self.exts.get(),
             "auth": dict(AUTH)}
        try:
            with open(SETTINGS, "w", encoding="utf-8") as fh: json.dump(s, fh, indent=2)
        except OSError:
            pass

    def on_close(self):
        self.stop_flag.set()
        self.save_settings()
        self.root.destroy()

    def pick(self):
        d = filedialog.askdirectory()
        if d: self.outdir.set(d)

    def start(self):
        lines = self.links.get("1.0", "end").splitlines()
        parsed = [p for p in map(parse_link, lines) if p]
        if not parsed:
            messagebox.showwarning("Niente da fare", "Nessun link valido."); return
        self.save_settings()
        self.stop_flag.clear()
        exts = tuple("." + e.strip().lower().lstrip(".") for e in self.exts.get().split(",") if e.strip())
        threading.Thread(target=self.run, args=(parsed, exts, self.originals.get(),
                         self.outdir.get(), self.workers.get()), daemon=True).start()

    def stop(self):
        self.stop_flag.set()

    def clear_done(self):
        for iid in self.tree.get_children():
            if self.tree.set(iid, "status").startswith("✔"):
                self.tree.delete(iid); self.jobs.pop(iid, None)

    def run(self, parsed, exts, originals, outdir, nworkers):
        jobs = []
        for ident, path in parsed:
            try:
                if path:
                    jobs.append((ident, path, 0))
                else:
                    files = list_files(ident, originals, exts)
                    if not files: self.q.put(("error", f"{ident}: nessun file corrisponde ai filtri"))
                    jobs += [(ident, n, s) for n, s in files]
            except Exception as e:
                self.q.put(("error", f"{ident}: {e}"))
        for j in jobs:
            self.q.put(("add", j))
        with ThreadPoolExecutor(max_workers=nworkers) as ex:
            for j in jobs: ex.submit(self.download, j, outdir)

    def download(self, job, outdir):
        ident, name, size = job
        iid = f"{ident}/{name}"
        if self.stop_flag.is_set():
            self.q.put(("upd", iid, None, None, "Fermato")); return
        url = f"https://archive.org/download/{ident}/{urllib.parse.quote(name)}"
        dest = os.path.join(outdir, ident, *name.split("/"))
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        for attempt in range(1, 4):
            try:
                have = os.path.getsize(dest) if os.path.exists(dest) else 0
                if size and have >= size:
                    self.q.put(("upd", iid, size, size, "✔ Già presente")); return
                req = urllib.request.Request(url, headers=headers({"Range": f"bytes={have}-"} if have else None))
                with urllib.request.urlopen(req, timeout=60) as r:
                    if r.status != 206: have = 0          # server non supporta resume: riparte
                    total = have + int(r.headers.get("Content-Length") or 0)
                    done, t0, last, last_done = have, time.time(), 0, have
                    with open(dest, "ab" if have else "wb") as fh:
                        while True:
                            if self.stop_flag.is_set():
                                self.q.put(("upd", iid, done, total, "Fermato (riprendibile)")); return
                            chunk = r.read(CHUNK)
                            if not chunk: break
                            fh.write(chunk); done += len(chunk)
                            now = time.time()
                            if now - last > 0.4:
                                spd = (done - last_done) / (now - last) if last else 0
                                self.q.put(("upd", iid, done, total, f"{human(spd)}/s"))
                                last, last_done = now, done
                self.q.put(("upd", iid, done, total or done, "✔ Completato")); return
            except urllib.error.HTTPError as e:
                if e.code == 416 and have:            # range oltre la fine: file già completo
                    self.q.put(("upd", iid, have, have, "✔ Già presente")); return
                if e.code in (401, 403):              # inutile ritentare
                    why = "solo in prestito o riservato" if AUTH else "serve accedere"
                    self.q.put(("upd", iid, None, None, f"✘ Accesso negato ({why})")); return
                self.q.put(("upd", iid, None, None, f"Errore ({attempt}/3): {e}"))
                time.sleep(3 * attempt)
            except Exception as e:
                self.q.put(("upd", iid, None, None, f"Errore ({attempt}/3): {e}"))
                time.sleep(3 * attempt)
        self.q.put(("upd", iid, None, None, "✘ Fallito"))

    def poll(self):
        try:
            while True:
                msg = self.q.get_nowait()
                if msg[0] == "add":
                    ident, name, size = msg[1]; iid = f"{ident}/{name}"
                    if not self.tree.exists(iid):
                        self.tree.insert("", "end", iid=iid, values=(iid, human(size) if size else "?", "0%", "", "In coda"))
                    self.jobs[iid] = [0, size]
                elif msg[0] == "upd":
                    _, iid, done, total, status = msg
                    if not self.tree.exists(iid): continue
                    if done is not None:
                        self.jobs[iid] = [done, total]
                        pct = f"{done / total * 100:.1f}%" if total else human(done)
                        self.tree.set(iid, "progress", pct)
                        if total: self.tree.set(iid, "size", human(total))
                    is_speed = status.endswith("/s")
                    self.tree.set(iid, "speed", status if is_speed else "")
                    self.tree.set(iid, "status", "Download…" if is_speed else status)
                elif msg[0] == "error":
                    messagebox.showerror("Errore", msg[1])
        except queue.Empty:
            pass
        d = sum(j[0] for j in self.jobs.values()); t = sum(j[1] or 0 for j in self.jobs.values())
        if t:
            self.overall["value"] = min(100, d / t * 100)
            self.total_lbl.config(text=f"{human(d)} / {human(t)}")
        self.root.after(200, self.poll)

if __name__ == "__main__":
    root = tk.Tk(); App(root); root.mainloop()