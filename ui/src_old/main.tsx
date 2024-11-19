import React from "react";
import ReactDOM from "react-dom/client";

import "@/styles/tailwind.css"
import { Frame } from "@/components/frame";


ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Frame />
  </React.StrictMode>,
);
