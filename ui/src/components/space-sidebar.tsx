import { Link, useParams } from "react-router-dom"
import { Home, User } from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { useQueryListSpaces, useQueryPrograms, useQueryTables } from "@/api";
import { LoadingSpinner } from "@/components/ui/loading";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Uuid } from "@/types";


export function SpaceSidebar() {
  const { spaceId = "" }  = useParams<{ spaceId: Uuid }>();
  const { isLoading: isLoadingSpaces, data: spaces } = useQueryListSpaces({ offset: 0, limit: 25 });
  const { isLoading: isLoadingTables, data: tables } = useQueryTables({ spaceId, offset: 0, limit: 25 });
  const { isLoading: isLoadingPrograms, data: programs } = useQueryPrograms({ spaceId, offset: 0, limit: 25 });


  const items = [
    {
      title: "Home",
      url: `/spaces/${spaceId}`,
      icon: Home,
    },
    {
      title: "People",
      url: `/spaces/${spaceId}/people`,
      icon: User,
    },
  ]

  const space = spaces?.find((space) => space.id === spaceId);
  
  return (
    <Sidebar>
      <SidebarHeader>
        <DropdownMenu>
          <DropdownMenuTrigger>{space?.name}</DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuLabel>Space</DropdownMenuLabel>
            <DropdownMenuSeparator />
            {spaces?.map((space) => (
              <DropdownMenuCheckboxItem checked={(spaceId == space.id)} key={space.id}>
                {space.name}
              </DropdownMenuCheckboxItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarHeader>
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
          <Link to={`/spaces/${spaceId}/programs`}>
            <SidebarGroupLabel>Programs</SidebarGroupLabel>
          </Link>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild>
                  <Link to={`/spaces/${spaceId}/programs`}>
                    <span>Programs</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              {isLoadingPrograms && <LoadingSpinner />}
              {programs?.map((program) => (
                <SidebarMenuItem key={program.id}>
                  <SidebarMenuButton asChild>
                    <Link to={`/spaces/${spaceId}/programs/${program.id}`}>
                      <span>{program.manifest.name}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        <SidebarGroup>
          <Link to={`/spaces/${spaceId}/tables`}>
            <SidebarGroupLabel>Tables</SidebarGroupLabel>
          </Link>
          <SidebarGroupContent>
            <SidebarMenu>
              {isLoadingTables && <LoadingSpinner />}
              {tables?.map((table) => (
                <SidebarMenuItem key={table.content.hash}>
                  <SidebarMenuButton asChild>
                    <Link to={`/spaces/${spaceId}/tables/${table.content.hash}`}>
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
