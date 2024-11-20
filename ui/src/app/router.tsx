import { RouterProvider, createBrowserRouter } from 'react-router-dom'

// import NotFound from "./layouts/NotFound"

const router = createBrowserRouter([
  {
    path: '/',
    lazy: () => import('./home'),
  },
  {
    path: '/:space',
    lazy: () => import('./space/layout'),
    children: [
      {
        path: '',
        lazy: () => import('./space/tables'),
      },
      {
        path: 'tables',
        lazy: () => import('./space/tables'),
      },
      {
        path: 'tables/:tableHash',
        lazy: () => import('./space/table')
      },
      {
        path: 'people',
        lazy: () => import('./space/people'),
      },
      {
        path: 'programs',
        lazy: () => import('./space/programs'),
      },
      {
        path: 'programs/:programId',
        lazy: () => import('./space/program'),
      }
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
