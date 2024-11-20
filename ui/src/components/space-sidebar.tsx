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
import { useQueryPrograms, useQueryTables } from "@/api";
import { LoadingSpinner } from "@/components/ui/loading";


export function SpaceSidebar() {
  const { space = "" }  = useParams<{ space: string }>();
  const { isLoading: isLoadingTables, data: tables } = useQueryTables({ space, offset: 0, limit: 25 });
  const { isLoading: isLoadingPrograms, data: programs } = useQueryPrograms({ space, offset: 0, limit: 25 });

  const items = [
    {
      title: "Home",
      url: `/spaces/${space}`,
      icon: Home,
    },
    {
      title: "Programs",
      url: `/spaces/${space}/programs`,
      icon: Bot
    },
    {
      title: "People",
      url: `/spaces/${space}/people`,
      icon: User,
    },
    {
      title: "Tables",
      url: `/spaces/${space}/tables`,
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
        <SidebarGroup>
          <Link to={`/spaces/${space}/programs`}>
            <SidebarGroupLabel>Programs</SidebarGroupLabel>
          </Link>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild>
                  <Link to={`/spaces/${space}/programs`}>
                    <span>Programs</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              {isLoadingPrograms && <LoadingSpinner />}
              {programs?.map((program) => (
                <SidebarMenuItem key={program.content.hash}>
                  <SidebarMenuButton asChild>
                    <Link to={`/spaces/${space}/programs/${program.content.hash}`}>
                      <span>{program.manifest.name}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        <SidebarGroup>
          <Link to={`/spaces/${space}/tables`}>
            <SidebarGroupLabel>Tables</SidebarGroupLabel>
          </Link>
          <SidebarGroupContent>
            <SidebarMenu>
              {isLoadingTables && <LoadingSpinner />}
              {tables?.map((table) => (
                <SidebarMenuItem key={table.content.hash}>
                  <SidebarMenuButton asChild>
                    <Link to={`/spaces/${space}/tables/${table.content.hash}`}>
                      <span>{table.title}</span>
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
