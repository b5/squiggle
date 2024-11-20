import { useParams } from "react-router-dom"

import { useQueryUsers } from "@/api"
import { Loading } from "@/components/ui/loading"
import { Uuid } from "@/types";

export function Component() {
  const { spaceId = "" } = useParams<{ spaceId: Uuid }>();
  const { isLoading, data } = useQueryUsers({ spaceId, offset: 0, limit: -1 });
  
  if (isLoading) {
    return <Loading />
  }

  return (
    <div className="p-4">

    </div>
  )
}