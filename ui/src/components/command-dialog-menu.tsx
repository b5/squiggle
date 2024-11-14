import * as React from "react"
import {
  ArrowRight,
  Calculator,
  Calendar,
  CreditCard,
  Settings,
  Smile,
  User,
} from "lucide-react"
import { emit } from '@tauri-apps/api/event';

import {
  CommandDialog,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command"

export function CommandDialogMenu({ navigate }: { navigate: (to: string) => void }) {
  // const [open, setOpen] = React.useState(true)
  const [value, setValue] = React.useState("")
  const [searchValue, setSearchValue] = React.useState("")

  React.useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        // setOpen((open) => !open)
      } else if (e.key === "Escape") {
        emit('dismiss-ui', {})
      }
    }

    document.addEventListener("keydown", down)

    return () => document.removeEventListener("keydown", down)
  }, [])

  return (
    <CommandDialog open={true} onOpenChange={(newOpen)  => { if (!newOpen) { emit('dismiss-ui', {}) } }} value={value} onValueChange={(v) => { setValue(v); }}>
      <CommandInput value={searchValue} onValueChange={(v) => setSearchValue(v)} placeholder="Type a command or search..." />
      <CommandList>
        <CommandGroup>
          <CommandItem value={searchValue} onSelect={navigate}>
              <ArrowRight />
              <span>go</span>
          </CommandItem>
        </CommandGroup>
        <CommandGroup heading="Suggestions">
          <CommandItem value="calendar" onSelect={(_) => navigate("https://youtube.com")}>
            <Calendar />
            <span>Calendar</span>
          </CommandItem>
          <CommandItem value="emoji" onSelect={(_) => navigate("https://apple.com")}>
            <Smile />
            <span>Search Emoji</span>
          </CommandItem>
          <CommandItem value="https://n0.computer" onSelect={navigate}>
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
