import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { CardSpotlight } from "./theme/CardSpotlight";
import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CardSpotlight />
    <App />
  </StrictMode>,
);
