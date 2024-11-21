import { useState } from "react"
import { Plus, Trash2 } from 'lucide-react'

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Uuid } from "@/types"
import { useMutationSetSecrets } from "@/api"

export interface SecretsDialogProps {
  spaceId: Uuid;
  programId: Uuid;
  secrets?: Record<string, string>
}

export default function SecretsDialog({ spaceId, programId, secrets: initialSecrets }: SecretsDialogProps) {
  const mapped = initialSecrets ? Object.entries(initialSecrets).map(([key, value]) => ({ key, value })) : []
  let saveSecrets = useMutationSetSecrets()

  const [secrets, setSecrets] = useState(mapped)

  const addSecret = () => {
    setSecrets([...secrets, { key: "", value: "" }])
  }

  const removeSecret = (index: number) => {
    setSecrets(secrets.filter((_, i) => i !== index))
  }

  const updateSecret = (index: number, field: "key" | "value", value: string) => {
    const updatedSecrets = [...secrets]
    updatedSecrets[index][field] = value
    setSecrets(updatedSecrets)
  }

  return (
    <Dialog onOpenChange={(open) => {
      if (!open) {
        saveSecrets({ spaceId, programId, secrets: Object.fromEntries(secrets.map(({ key, value }) => [key, value])) })
      }
    }}>
      <DialogTrigger asChild>
        <Button variant="outline">Edit Secret Settings</Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-[525px]">
        <DialogHeader>
          <DialogTitle>Secret Settings</DialogTitle>
          <DialogDescription>
            Add or modify secret environment variables. Click save when you're done.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4 max-h-[60vh] overflow-y-auto">
          {secrets.map((secret, index) => (
            <div key={index} className="grid grid-cols-[1fr_1fr_auto] items-center gap-4">
              <div>
                <Label htmlFor={`key-${index}`} className="sr-only">
                  Key
                </Label>
                <Input
                  id={`key-${index}`}
                  placeholder="KEY"
                  value={secret.key}
                  onChange={(e) => updateSecret(index, "key", e.target.value)}
                />
              </div>
              <div>
                <Label htmlFor={`value-${index}`} className="sr-only">
                  Value
                </Label>
                <Input
                  id={`value-${index}`}
                  placeholder="VALUE"
                  value={secret.value}
                  onChange={(e) => updateSecret(index, "value", e.target.value)}
                />
              </div>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => removeSecret(index)}
                className="h-10 w-10"
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
        <Button onClick={addSecret} variant="outline" className="w-full">
          <Plus className="mr-2 h-4 w-4" /> Add Secret
        </Button>
        <DialogFooter>
          <Button type="submit">Save changes</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}