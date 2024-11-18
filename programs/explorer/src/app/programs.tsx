import { useQueryPrograms } from "@/api";
import { Loading } from "@/components/ui/loading";


export function Component() {
  const { isLoading, data } = useQueryPrograms({ offset: 0, limit: -1 });
  if (isLoading) {
    return <Loading />;
  }

  return (
    <div className="p-4">
      <h1>Programs</h1>
      {data?.map((program, i) => (
        <div key={i} className="p-2 border-b">
          {JSON.stringify(program)}
        </div>
      ))}
    </div>
  )
}