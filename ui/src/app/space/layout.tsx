import { Outlet } from "react-router-dom"

import { SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
import { SpaceSidebar } from "@/components/space-sidebar"
import { CommandDialogMenu } from "@/components/command-dialog-menu"
import { RequireSpace } from "@/components/require-space"
 
export function Component() {
  return (
    <RequireSpace>
      <SidebarProvider>
        <SpaceSidebar />
        <CommandDialogMenu />
        <main className="w-full h-screen overflow-y-auto">
          <div className="p-4">
            <SidebarTrigger />
          </div>
          <Outlet />
        </main>
      </SidebarProvider>
    </RequireSpace>
  )
}