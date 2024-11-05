
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


  let body = JSON.parse(res.body);
  const { event_create } = Host.getFunctions();

  let schmea = Memory.fromString("7vtxfvpypm2ha7c5hpmy3t2e26glim256ebphxxfar6jqrzzwpya");;
  let mem = Memory.fromJsonObject(body);
  let offset = event_create(schmea.offset, mem.offset);
  const event = Memory.find(offset).readJsonObject()
  
  Host.outputString(JSON.stringify(event));
}
