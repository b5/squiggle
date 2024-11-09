import { useListSchemas } from "@/api"


export function Component() {
  const { isLoading, data } = useListSchemas({ offset: 0, limit: 10 });
  if (isLoading) {
    return <div>Loading...</div>
  }

  return (
    <div className="p-4">
      <p>Huh?</p>
      {data?.map((schema) => {
        return (
          <div key={schema.id} className="p-2 border-b">
            <h2>{schema.name}</h2>
            <p>{schema.description}</p>
          </div>
        )
      })}
    </div>
  )
}