import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// Mantine's own stylesheet first, so index.css (which pulls in Bulma) loads
// after it in cascade order -- matches Mantine's own guidance for combining
// it with another CSS framework.
import "@mantine/core/styles.css";
import "./index.css";

const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Root element #root not found');
ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
