import { useQueryRows, useQuerySchema } from "@/api";
import { Loading } from "@/components/ui/loading";
import { useParams } from "react-router-dom"


export function Component() {
  const { schemaHash = "" }  = useParams<{ schemaHash: string }>();
  const schemaEnv = useQuerySchema({ schema: schemaHash });
  const { isLoading, data } = useQueryRows({ schema: schemaHash, offset: 0, limit: -1 });

  
  if (isLoading) {
    return <Loading />
  }

  return (
    <div className="p-4">
      <h1>Data</h1>
      {schemaEnv.data && JSON.stringify(schemaEnv.data.content.value)}
      {data?.map((e,i) => { 
        return (
          <div key={i} className="p-2 border-b">
            {JSON.stringify(e.content.value)}
          </div>
        )
      })}
    </div>
  )
}