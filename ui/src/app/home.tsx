import { Link } from "react-router-dom";

import { useListSpaces } from "@/api"
import { Loading } from "@/components/ui/loading";

export function Component() {
  const { isLoading, data } = useListSpaces({ offset: 0, limit: -1 }); 

  if (isLoading) {
    return <Loading />
  }

  return (
    <div className="p-4">
      <h1>Home</h1>
      {data?.map((space, i) => {
        return (
          <div key={i} className="p-2 border-b">
            <Link to={`/spaces/${space.name}`} className="cursor-pointer">{space.name}</Link>
          </div>
        )
      })}
    </div>
  )
}