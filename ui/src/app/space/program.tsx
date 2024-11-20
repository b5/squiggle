import { useQueryProgram, useRunProgramMutation} from "@/api";
import { Button } from "@/components/ui/button";
import { Loading } from "@/components/ui/loading";
import { useParams } from "react-router-dom";


export function Component() {
  const { space = "", programId = "" } = useParams<{ space: string, programId: string }>();
  const { isLoading, data } = useQueryProgram({ space, programId });
  const run = useRunProgramMutation()
  
  if (isLoading) {
    return <Loading />;
  }

  return (
    <div className="p-4">
      <h1>{data?.manifest.name}</h1>
      <Button onClick={() => run({ space, programId, author: "me", environment: {} })}>Run</Button>
    </div>
  )
}