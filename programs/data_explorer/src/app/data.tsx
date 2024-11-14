import { Link } from "react-router-dom";

import { useListSchemas } from "@/api"
import { Loading } from "@/components/ui/loading";


export function Component() {
  const { isLoading, data } = useListSchemas({ offset: 0, limit: 10 });

  if (isLoading) {
    return <Loading />
  }

  return (
    <div className="p-4">
      <h1>Data</h1>
      {data?.map((schema) => {
        return (
          <div key={schema.hash} className="p-2 border-b">
            <Link to={`/data/${schema.hash}`} className="cursor-pointer">{schema.title}</Link>
          </div>
        )
      })}
    </div>
  )
}