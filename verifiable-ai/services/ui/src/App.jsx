import { useState } from "react";
import "./styles.css";
import { OperatorDashboard } from "./components/OperatorDashboard";
import { Studio } from "./components/Studio";

export default function App() {
  const [view, setView] = useState("studio"); // 'studio' | 'operator'

  return (
    <div className="app">
      <div className="bgGlow" />

      <div className="topSwitch">
        <button
          className={`switchBtn ${view === "studio" ? "active" : ""}`}
          onClick={() => setView("studio")}
        >
          Studio
        </button>
        <button
          className={`switchBtn ${view === "operator" ? "active" : ""}`}
          onClick={() => setView("operator")}
        >
          Operator
        </button>
      </div>

      {view === "studio" ? <Studio /> : <OperatorDashboard />}
    </div>
  );
}
