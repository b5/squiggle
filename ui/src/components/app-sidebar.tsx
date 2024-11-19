import { Link, useParams } from "react-router-dom"
import { Home, Bot, User, HardDrive } from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"


export function AppSidebar() {
  const { space = "" }  = useParams<{ space: string }>();

  const items = [
    {
      title: "Home",
      url: `/${space}`,
      icon: Home,
    },
    {
      title: "Programs",
      url: `/${space}/programs`,
      icon: Bot
    },
    {
      title: "People",
      url: `/${space}/people`,
      icon: User,
    },
    {
      title: "Data",
      url: `/${space}/tables`,
      icon: HardDrive,
    }
  ]
  
  return (
    <Sidebar>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Application</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {items.map((item) => (
                <SidebarMenuItem key={item.title}>
                  <SidebarMenuButton asChild>
                    <Link to={item.url}>
                      <item.icon />
                      <span>{item.title}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  )
}
