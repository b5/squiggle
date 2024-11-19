import { ColumnDef } from "@tanstack/react-table"
import { useParams } from "react-router-dom"

import { useQueryRows, useQuerySchema } from "@/api";
import { Loading } from "@/components/ui/loading";
import { DataTable } from "@/components/data-table";
import { Badge } from "@/components/ui/badge"
import { Checkbox } from "@/components/ui/checkbox"
import { labels, priorities, statuses } from "@/data/model"
import { Task } from "@/data/schema"
import { DataTableColumnHeader } from "@/components/data-table-column-header"
import { DataTableRowActions } from "@/components/data-table-row-actions"


export function Component() {
  const { space = "", schemaHash = "" }  = useParams<{ space: string, schemaHash: string }>();
  const schemaEnv = useQuerySchema({ space, schema: schemaHash });
  const { isLoading, data } = useQueryRows({ space, schema: schemaHash, offset: 0, limit: -1 });
  
  if (isLoading || schemaEnv.isLoading) {
    return <Loading />
  }

  const schema = schemaEnv.data?.content.value
  const mapper = mapData(schema || {});

  return (
    <div className="p-4">
      <h1>Data</h1>
      {schema && 
        <DataTable data={data?.map(mapper) || []} columns={arrayRowColumns(schema)} />
      }
    </div>
  )
}

function isArraySchema(schema: Record<string, any>): boolean {
  return schema.type === "array"
}

function mapData(schema: Object): (row: any) => any {
  if (isArraySchema(schema)) {
    return (row) => {
      row = row.content.value

      const result = row.reduce((acc, item, i) => {
        acc[schema.prefixItems[i].title] = item
        return acc
      }, {})
      return result
    }}
  return (row) => row
}

function arrayRowColumns(schema: Record<string, any>): ColumnDef<any>[] {
  if (!schema.prefixItems) {
    console.log("No prefix items")
    console.log(schema)
    return []
  }

  return schema.prefixItems.map((column, i) => ({
    accessorKey: `${column.title}`,
    header: ({ column }) => (
      <DataTableColumnHeader column={column.title} title={column.title} />
    ),
    cell: ({ row }) => <div className="w-[80px]">{row.getValue(column.title)}</div>,
    enableSorting: false,
    enableHiding: false,
  } as ColumnDef<Record<string,any>>)) || []
}


// const columns: ColumnDef<any>[] = [
//   {
//     id: "select",
//     header: ({ table }) => (
//       <Checkbox
//         checked={
//           table.getIsAllPageRowsSelected() ||
//           (table.getIsSomePageRowsSelected() && "indeterminate")
//         }
//         onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
//         aria-label="Select all"
//         className="translate-y-[2px]"
//       />
//     ),
//     cell: ({ row }) => (
//       <Checkbox
//         checked={row.getIsSelected()}
//         onCheckedChange={(value) => row.toggleSelected(!!value)}
//         aria-label="Select row"
//         className="translate-y-[2px]"
//       />
//     ),
//     enableSorting: false,
//     enableHiding: false,
//   },
//   {
//     accessorKey: "foo",
//     cell: ({ row }) => <div className="w-[80px]">{row.getValue("foo")}</div>,
//     enableSorting: false,
//     enableHiding: false,
//   }
//   {
//     accessorKey: "id",
//     header: ({ column }) => (
//       <DataTableColumnHeader column={column} title="Task" />
//     ),
//     cell: ({ row }) => <div className="w-[80px]">{row.getValue("id")}</div>,
//     enableSorting: false,
//     enableHiding: false,
//   },
//   {
//     accessorKey: "title",
//     header: ({ column }) => (
//       <DataTableColumnHeader column={column} title="Title" />
//     ),
//     cell: ({ row }) => {
//       const label = labels.find((label) => label.value === row.original.label)

//       return (
//         <div className="flex space-x-2">
//           {label && <Badge variant="outline">{label.label}</Badge>}
//           <span className="max-w-[500px] truncate font-medium">
//             {row.getValue("title")}
//           </span>
//         </div>
//       )
//     },
//   },
//   {
//     accessorKey: "status",
//     header: ({ column }) => (
//       <DataTableColumnHeader column={column} title="Status" />
//     ),
//     cell: ({ row }) => {
//       const status = statuses.find(
//         (status) => status.value === row.getValue("status")
//       )

//       if (!status) {
//         return null
//       }

//       return (
//         <div className="flex w-[100px] items-center">
//           {status.icon && (
//             <status.icon className="mr-2 h-4 w-4 text-muted-foreground" />
//           )}
//           <span>{status.label}</span>
//         </div>
//       )
//     },
//     filterFn: (row, id, value) => {
//       return value.includes(row.getValue(id))
//     },
//   },
//   {
//     accessorKey: "priority",
//     header: ({ column }) => (
//       <DataTableColumnHeader column={column} title="Priority" />
//     ),
//     cell: ({ row }) => {
//       const priority = priorities.find(
//         (priority) => priority.value === row.getValue("priority")
//       )

//       if (!priority) {
//         return null
//       }

//       return (
//         <div className="flex items-center">
//           {priority.icon && (
//             <priority.icon className="mr-2 h-4 w-4 text-muted-foreground" />
//           )}
//           <span>{priority.label}</span>
//         </div>
//       )
//     },
//     filterFn: (row, id, value) => {
//       return value.includes(row.getValue(id))
//     },
//   },
//   {
//     id: "actions",
//     cell: ({ row }) => <DataTableRowActions row={row} />,
//   },
// ]