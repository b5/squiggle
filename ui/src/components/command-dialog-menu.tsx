import * as React from "react"
import { useNavigate, useParams } from "react-router-dom"
import {
  Bot,
  Table,
} from "lucide-react"

import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import { useEventSearch } from "@/api"
import { schemaId, Uuid } from "@/types"
import { Loading } from "./ui/loading"

export function CommandDialogMenu() {
  const { spaceId = "" } = useParams<{ spaceId: Uuid }>()
  const navigate = useNavigate()
  const [open, setOpen] = React.useState(false)
  const [query, setQuery] = React.useState("")
  const { isLoading, data } = useEventSearch(spaceId, query, 0, 1000);

  React.useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        setOpen((open) => !open)
      } else if (e.key === "Enter" && open) {
        setOpen(false)
        return
      }
    }

    document.addEventListener("keydown", down)
    return () => document.removeEventListener("keydown", down)
  }, [])

  const navHandler = (to: string) => () => navigate(to)

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput placeholder="Type a command or search..." value={query} onValueChange={setQuery} />
      <CommandList>
        {(data || isLoading) && <CommandGroup heading="Search Results" forceMount>
          {isLoading && <Loading />}
          {data?.map((event) => {
            const schId = schemaId(event)
            return (
            <CommandItem key={event.id} value={`/spaces/${spaceId}/tables/${schId}#${event.id}`} forceMount onSelect={(path) => {
              console.log(path)
              navigate(path)
            }}>
              <span>{event.id}</span>
              <span>{event.createdAt}</span>
            </CommandItem>
          )})}
          {(data?.length === 0) && <CommandEmpty>No results found.</CommandEmpty>}
        </CommandGroup>}
        <CommandGroup heading='Places'>
          <CommandItem onSelect={navHandler(`/spaces/${spaceId}/programs`)}>
            <Bot />
            <span>Programs</span>
          </CommandItem>
          <CommandItem onSelect={navHandler(`/spaces/${spaceId}/tables`)}>
            <Table />
            <span>Tables</span>
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  )
}
