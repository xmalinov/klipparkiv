import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { I18nProvider } from "./i18n";
import { SettingsProvider } from "./providers/SettingsProvider";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsProvider>
      <I18nProvider>
        <App />
      </I18nProvider>
    </SettingsProvider>
  </React.StrictMode>,
);
