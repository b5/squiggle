import * as React from "react"
import { useNavigate, useParams } from "react-router-dom"
import {
  Calculator,
  Calendar,
  CreditCard,
  Settings,
  Smile,
  User,
} from "lucide-react"

import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command"
import { useEventSearch } from "@/api"
import { schemaId } from "@/types"
import { Loading } from "./ui/loading"

export function CommandDialogMenu() {
  const { space = "" } = useParams<{ space: string }>()
  const navigate = useNavigate()
  const [open, setOpen] = React.useState(false)
  const [query, setQuery] = React.useState("")
  const { isLoading, data } = useEventSearch(space, query, 0, 1000);

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

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput placeholder="Type a command or search..." value={query} onValueChange={setQuery} />
      <CommandList>
        {(data || isLoading) && <CommandGroup heading="Search Results" forceMount>
          {isLoading && <Loading />}
          {data?.map((event) => {
            const schId = schemaId(event)
            return (
            <CommandItem key={event.id} value={`/${space}/tables/${schId}#${event.id}`} forceMount onSelect={(path) => {
              console.log(path)
              navigate(path)
            }}>
              <span>{event.id}</span>
              <span>{event.createdAt}</span>
            </CommandItem>
          )})}
          {(data?.length === 0) && <CommandEmpty>No results found.</CommandEmpty>}
        </CommandGroup>}
        <CommandGroup heading="Suggestions">
          <CommandItem>
            <Calendar />
            <span>Calendar</span>
          </CommandItem>
          <CommandItem>
            <Smile />
            <span>Search Emoji</span>
          </CommandItem>
          <CommandItem>
            <Calculator />
            <span>Calculator</span>
          </CommandItem>
        </CommandGroup>
        <CommandSeparator />
        <CommandGroup heading="Settings">
          <CommandItem>
            <User />
            <span>Profile</span>
            <CommandShortcut>⌘P</CommandShortcut>
          </CommandItem>
          <CommandItem>
            <CreditCard />
            <span>Billing</span>
            <CommandShortcut>⌘B</CommandShortcut>
          </CommandItem>
          <CommandItem>
            <Settings />
            <span>Settings</span>
            <CommandShortcut>⌘S</CommandShortcut>
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  )
}
