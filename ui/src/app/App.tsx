import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import "@/styles/tailwind.css"
import Layout from "@/app/Layout";
import People from "@/app/People";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [flowResult, setFlowResult] = useState({});

  async function listUsers() {
    const users = await invoke("list_users");
    console.log(users);
  }

  async function runFlow() {
    let res = await invoke("run_flow", { path: "../../node/tests/wasm.toml" }).catch((e) => {
      console.error(e);
    });
    console.log(res);
    setFlowResult(res);
  }

  return (
    <Layout>
      <People />
    </Layout>
  );
}

export default App;
