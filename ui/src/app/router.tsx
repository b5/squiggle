import { RouterProvider, createBrowserRouter } from 'react-router-dom'

// import NotFound from "./layouts/NotFound"

const router = createBrowserRouter([
  {
    path: '/',
    lazy: () => import('./Layout'),
    children: [
      {
        path: '',
        lazy: () => import('./home'),
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
        path: 'data',
        lazy: () => import('./data'),
      },
      {
        path: 'bots',
        lazy: () => import('./bots'),
      },
      {
        path: 'browse',
        lazy: () => import('./webpage'),
      }
      // {
      //   path: 'settings',
      //   lazy: () => import('./settings/settings'),
      // },
      // {
      //   path: 'content',
      //   lazy: () => import('./content/page'),
      // },
      // {
      //   path: 'content/:id',
      //   lazy: () => import('./content/content-item'),
      // },
      // {
      //   path: 'devices',
      //   lazy: () => import('./devices/devices'),
      // },
      // {
      //   path: 'devices/:nodeId',
      //   lazy: () => import('./devices/device'),
      // },
    ],
  },
  // {
  //   path: '/login',
  //   lazy: () => import('../app/session/login'),
  // },
  // {
  //   path: '/signup',
  //   lazy: () => import('../app/session/signup'),
  // },
  {
    path: '*',
    element: <h1>Not Found</h1>,
  },
])

export default function Router() {
  return <RouterProvider router={router} future={{ v7_startTransition: true }} />
}
