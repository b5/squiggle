import { invoke, InvokeArgs } from "@tauri-apps/api/core";
import { emit } from '@tauri-apps/api/event';

import { SidebarProvider } from "@/components/ui/sidebar"
import { AppSidebar } from "@/components/app-sidebar"
import { CommandDialogMenu } from "@/components/command-dialog-menu"


const DEFAULT_PAGE = "http://localhost:8080/collection/5d0449028d46eebd75e45128eac7d522eec50ba3f5f1a70fbcd95dd1e59871f9/index.html"

export function Frame() {

  return (
    <div onClick={() => {
      emit("dismiss-ui", {})
    }}>
        {/* <SidebarProvider>
          <AppSidebar />
        </SidebarProvider> */}
      <CommandDialogMenu navigate={(url) => {
        console.log("navigating to", url);
        invoke("navigate", { url } as InvokeArgs)
      }} />
    </div>
  )
}