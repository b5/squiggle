import { useQueryListSpaces } from "@/api";
import { Uuid } from "@/types";
import { Navigate, useParams } from "react-router-dom";
import { Loading } from "./ui/loading";


export function RequireSpace({ children }: { children: React.ReactNode }) {
  const { spaceId = "" }  = useParams<{ spaceId: Uuid }>();
  const { isLoading, data } = useQueryListSpaces({ offset: 0, limit: -1 });

  if (isLoading) {
    return <Loading />
  } else if (!data || !data.find((space) => space.id === spaceId)) {
    return <Navigate to="/spaces" />
  }

  return children;
}