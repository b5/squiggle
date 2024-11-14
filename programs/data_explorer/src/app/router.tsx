import { RouterProvider, createHashRouter } from 'react-router-dom'

// import NotFound from "./layouts/NotFound"

const router = createHashRouter([
  {
    path: '/',
    lazy: () => import('./layout'),
    children: [
      {
        path: '',
        lazy: () => import('./tables'),
      },
      {
        path: 'people',
        lazy: () => import('./people'),
      },
      {
        path: 'data/:schemaHash',
        lazy: () => import('./table')
      },
      {
        path: 'bots',
        lazy: () => import('./bots'),
      },
    ],
  },
  {
    path: '*',
    element: <h1>Not Found</h1>,
  },
])

export default function Router() {
  return <RouterProvider router={router} future={{ v7_startTransition: true }} />
}
