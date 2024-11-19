import { useParams } from "react-router-dom"

import { useQueryUsers } from "@/api"
import { Loading } from "@/components/ui/loading"

export function Component() {
  const { space = "", schemaHash = "" } = useParams<{ space: string, schemaHash: string }>();
  const { isLoading, data } = useQueryUsers({ space, offset: 0, limit: -1 });
  
  if (isLoading) {
    return <Loading />
  }

  return (
    <div className="p-4">

    </div>
  )
}