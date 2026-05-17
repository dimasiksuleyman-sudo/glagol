import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";

import App from "./App";
import { CredentialsProvider } from "@/contexts/CredentialsContext";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <CredentialsProvider>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </CredentialsProvider>
  </React.StrictMode>,
);
