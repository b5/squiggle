import { useQueryPrograms } from "@/api";
import { Loading } from "@/components/ui/loading";
import { Link, useParams } from "react-router-dom";


export function Component() {
  const { space = "" } = useParams<{ space: string }>();
  const { isLoading, data } = useQueryPrograms({ space, offset: 0, limit: -1 });
  
  if (isLoading) {
    return <Loading />;
  }

  return (
    <div className="p-4">
      <h1>Programs</h1>
      {data?.map((program, i) => (
        <Link key={i} to={`/spaces/${space}/programs/${program.id}`} className="p-2 border-b block">
          <h3>{program.manifest.name}</h3>
        </Link>
      ))}
    </div>
  )
}