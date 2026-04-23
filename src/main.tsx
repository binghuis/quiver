import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Toaster } from "@/shared/ui/sonner";
import "./globals.css";

document.documentElement.classList.add("dark");
document.addEventListener("contextmenu", (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
    <Toaster theme="dark" position="bottom-right" />
  </React.StrictMode>,
);
