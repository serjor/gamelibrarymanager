import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { bootstrapTheme } from "./features/shell/theme";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("#root is absent in index.html");

bootstrapTheme();

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
