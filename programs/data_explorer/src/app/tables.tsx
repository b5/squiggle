import { Link } from "react-router-dom";

import { useQuerySchemas } from "@/api"
import { Loading } from "@/components/ui/loading";


export function Component() {
  const { isLoading, data } = useQuerySchemas({ offset: 0, limit: 10 });

  if (isLoading) {
    return <Loading />
  }

  return (
    <div className="p-4">
      <div className="pb-4 mb-4 border-b">
        <h1 className="text-xl font-bold">Local Data</h1>
        <p className="text-sm">data stored on your local device</p>
      </div>
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