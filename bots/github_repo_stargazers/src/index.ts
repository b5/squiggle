
export function meta() {
  const data = {
    name: "github_repo_stargazers",
    version: "0.0.1",
  };

  Host.outputString(JSON.stringify(data));
}

export function main() {
  Host.outputString(`Hello, ${Host.inputString()}`)

  const req: HttpRequest = {
    url: "https://postman-echo.com/get",
    method: "GET",
  };
  let res = Http.request(req);
  if (res.status !== 200) {
    return -2;
  }

  let body = JSON.stringify(res.body);
  Host.outputString(body);
}
