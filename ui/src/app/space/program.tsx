import { format } from 'date-fns'
import { ExternalLink, Github, Home, Package, Calendar, User } from 'lucide-react'
import { useParams } from "react-router-dom";

import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Separator } from "@/components/ui/separator"
import { useQueryProgram, useMutationRunProgram, useQuerySecrets} from "@/api";
import SecretsDialog from "@/components/secrets-dialog";
import { Loading } from "@/components/ui/loading";
import { Uuid } from "@/types";
import { Program } from "@/types";


export function Component() {
  const { spaceId = "", programId = "" } = useParams<{ spaceId: Uuid, programId: Uuid }>();
  const { isLoading, data } = useQueryProgram({ spaceId, programId });
  const run = useMutationRunProgram()
  const { isLoading: isLoadingSecrets, data: secrets } = useQuerySecrets({ spaceId, programId });
  
  if (isLoading) {
    return <Loading />;
  }

  return (
    <div className="p-4">
      {data && <ProgramDetails program={data} />}
      <div className="container mx-auto py-10 px-4 sm:px-6 lg:px-8">
        <div className="max-w-3xl mx-auto">
          {!isLoadingSecrets && <SecretsDialog spaceId={spaceId} programId={programId} secrets={secrets} />}
          <Button onClick={() => run({ spaceId, programId, author: "me", environment: {} })}>Run</Button>
        </div>
      </div>
    </div>
  )
}

export default function ProgramDetails({ program }: { program: Program }) {
  return (
    <div className="container mx-auto py-10 px-4 sm:px-6 lg:px-8">
      <div className="max-w-3xl mx-auto">
        <div className="flex justify-between items-start mb-6">
          <div>
            <h1 className="text-3xl font-bold">{program.manifest.name}</h1>
            <p className="text-xl text-muted-foreground">Version {program.manifest.version}</p>
          </div>
          <Badge variant="outline" className="text-lg py-1 px-3">
            {program.manifest.license || 'No License'}
          </Badge>
        </div>

        <p className="text-lg mb-8">{program.manifest.description}</p>

        <div className="flex flex-wrap gap-4 mb-8">
          {program.manifest.homepage && (
            <Button variant="outline" size="lg" asChild>
              <a href={program.manifest.homepage} target="_blank" rel="noopener noreferrer">
                <Home className="mr-2 h-5 w-5" />
                Homepage
              </a>
            </Button>
          )}
          {program.manifest.repository && (
            <Button variant="outline" size="lg" asChild>
              <a href={program.manifest.repository} target="_blank" rel="noopener noreferrer">
                <Github className="mr-2 h-5 w-5" />
                Repository
              </a>
            </Button>
          )}
        </div>

        <Separator className="my-8" />

        <div className="grid gap-6 mb-8">
          <div>
            <h2 className="text-xl font-semibold mb-2">Program Details</h2>
            <div className="grid gap-4">
              <div className="flex items-center">
                <Package className="mr-2 h-5 w-5 text-muted-foreground" />
                <span className="font-medium mr-2">Program Entry:</span>
                <code className="p-1 bg-muted rounded">{program.program_entry}</code>
              </div>
              {program.manifest.main && (
                <div className="flex items-center">
                  <ExternalLink className="mr-2 h-5 w-5 text-muted-foreground" />
                  <span className="font-medium mr-2">Main:</span>
                  <code className="p-1 bg-muted rounded">{program.manifest.main}</code>
                </div>
              )}
            </div>
          </div>
        </div>

        <Separator className="my-8" />

        <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4">
          <div className="flex items-center space-x-4">
            <Avatar className="h-12 w-12">
              <AvatarFallback className="text-lg">{program.author.slice(0, 2).toUpperCase()}</AvatarFallback>
            </Avatar>
            <div>
              <p className="text-sm font-medium">Author</p>
              <p className="text-muted-foreground">{program.author.slice(0, 16)}...</p>
            </div>
          </div>
          <div className="flex items-center space-x-2">
            <Calendar className="h-5 w-5 text-muted-foreground" />
            <div>
              <p className="text-sm font-medium">Created</p>
              <p className="text-muted-foreground">
                {format(new Date(program.createdAt * 1000), 'PPP')}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}