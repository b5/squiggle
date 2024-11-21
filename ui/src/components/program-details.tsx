import { format } from 'date-fns'
import { ExternalLink, Github, Home, Package } from 'lucide-react'

import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Separator } from "@/components/ui/separator"
import { Program } from '@/types'

export default function ProgramDetails({ program }: { program: Program }) {
  return (
    <div className="container mx-auto py-10">
      <Card className="w-full">
        <CardHeader>
          <div className="flex justify-between items-start">
            <div>
              <CardTitle className="text-2xl">{program.manifest.name}</CardTitle>
              <CardDescription>Version {program.manifest.version}</CardDescription>
            </div>
            <Badge variant="outline">{program.manifest.license || 'No License'}</Badge>
          </div>
        </CardHeader>
        <CardContent>
          <p className="text-muted-foreground mb-4">{program.manifest.description}</p>
          <div className="flex flex-wrap gap-4 mb-6">
            {program.manifest.homepage && (
              <Button variant="outline" size="sm" asChild>
                <a href={program.manifest.homepage} target="_blank" rel="noopener noreferrer">
                  <Home className="mr-2 h-4 w-4" />
                  Homepage
                </a>
              </Button>
            )}
            {program.manifest.repository && (
              <Button variant="outline" size="sm" asChild>
                <a href={program.manifest.repository} target="_blank" rel="noopener noreferrer">
                  <Github className="mr-2 h-4 w-4" />
                  Repository
                </a>
              </Button>
            )}
          </div>
          <Separator className="my-4" />
          <div className="grid gap-4">
            <div className="flex items-center">
              <Package className="mr-2 h-4 w-4 text-muted-foreground" />
              <span className="font-medium">Program Entry:</span>
              <code className="ml-2 p-1 bg-muted rounded">{program.program_entry}</code>
            </div>
            {program.manifest.main && (
              <div className="flex items-center">
                <ExternalLink className="mr-2 h-4 w-4 text-muted-foreground" />
                <span className="font-medium">Main:</span>
                <code className="ml-2 p-1 bg-muted rounded">{program.manifest.main}</code>
              </div>
            )}
          </div>
        </CardContent>
        <CardFooter className="flex justify-between items-center">
          <div className="flex items-center space-x-2">
            <Avatar>
              <AvatarFallback>{program.author.slice(0, 2).toUpperCase()}</AvatarFallback>
            </Avatar>
            <div>
              <p className="text-sm font-medium">Author</p>
              <p className="text-xs text-muted-foreground">{program.author.slice(0, 10)}...</p>
            </div>
          </div>
          <div className="text-right">
            <p className="text-sm font-medium">Created</p>
            <p className="text-xs text-muted-foreground">
              {format(new Date(program.createdAt * 1000), 'PPP')}
            </p>
          </div>
        </CardFooter>
      </Card>
    </div>
  )
}