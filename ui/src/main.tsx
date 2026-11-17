import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { installErrorLogging, reportError } from "./agent/errorLog";
import { CardSpotlight } from "./theme/CardSpotlight";
import "./index.css";

// Erros da interface vão para o log do app (#534).
installErrorLogging();

createRoot(document.getElementById("root")!, {
  onUncaughtError: (error) => {
    console.error(error);
    reportError("tela quebrou", error);
  },
  onCaughtError: (error) => {
    console.error(error);
    reportError("tela", error);
  },
}).render(
  <StrictMode>
    <CardSpotlight />
    <App />
  </StrictMode>,
);
