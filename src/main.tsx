import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// No browser context menu (Back, Refresh, Print…): it's an app, not a page. Text fields keep
// it for copy and paste; rows with their own menu open it on right-click.
document.addEventListener("contextmenu", (e) => {
  if (!(e.target as HTMLElement).closest("textarea,input:not([type=checkbox]):not([type=radio])")) e.preventDefault();
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
