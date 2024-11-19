import { Outlet } from "react-router-dom"

import { SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
import { AppSidebar } from "@/components/app-sidebar"
import { CommandDialogMenu } from "@/components/command-dialog-menu"
 
export function Component() {
  return (
    <SidebarProvider>
      <AppSidebar />
      <CommandDialogMenu />
      <main className="w-full h-screen overflow-y-auto">
        <div className="p-4">
          <SidebarTrigger />
        </div>
        <Outlet />
      </main>
    </SidebarProvider>
  )
}