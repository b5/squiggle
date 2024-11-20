import { Link, useParams } from "react-router-dom";

import { useQueryPrograms } from "@/api";
import { Loading } from "@/components/ui/loading";
import { Uuid } from "@/types";


export function Component() {
  const { spaceId = "" } = useParams<{ spaceId: Uuid }>();
  const { isLoading, data } = useQueryPrograms({ spaceId, offset: 0, limit: -1 });
  
  if (isLoading) {
    return <Loading />;
  }

  return (
    <div className="p-4">
      <h1>Programs</h1>
      {data?.map((program, i) => (
        <Link key={i} to={`/spaces/${spaceId}/programs/${program.id}`} className="p-2 border-b block">
          <h3>{program.manifest.name}</h3>
        </Link>
      ))}
    </div>
  )
}