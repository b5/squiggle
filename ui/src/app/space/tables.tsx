import { Link, useParams } from "react-router-dom";

import { useQueryTables } from "@/api"
import { Loading } from "@/components/ui/loading";
import { Uuid } from "@/types";


export function Component() {
  let { spaceId = "" }  = useParams<{ spaceId: Uuid }>();
  const { isLoading, data } = useQueryTables({ spaceId, offset: 0, limit: 10 });

  if (isLoading) {
    return <Loading />
  }

  return (
    <div className="p-4">
      <div className="pb-4 mb-4 border-b">
        <h1 className="text-xl font-bold">Local Data</h1>
        <p className="text-sm">data stored on your local device</p>
      </div>
      {data?.map((schema, i) => {
        return (
          <div key={i} className="p-2 border-b">
            <Link to={`/spaces/${spaceId}/tables/${schema.content.hash}`} className="cursor-pointer">{schema.title}</Link>
          </div>
        )
      })}
    </div>
  )
}