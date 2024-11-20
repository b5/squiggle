import { useQueryProgram, useRunProgramMutation} from "@/api";
import { Button } from "@/components/ui/button";
import { Loading } from "@/components/ui/loading";
import { Uuid } from "@/types";
import { useParams } from "react-router-dom";


export function Component() {
  const { spaceId = "", programId = "" } = useParams<{ spaceId: Uuid, programId: Uuid }>();
  const { isLoading, data } = useQueryProgram({ spaceId, programId });
  const run = useRunProgramMutation()
  
  if (isLoading) {
    return <Loading />;
  }

  return (
    <div className="p-4">
      <h1>{data?.manifest.name}</h1>
      <Button onClick={() => run({ spaceId, programId, author: "me", environment: {} })}>Run</Button>
    </div>
  )
}