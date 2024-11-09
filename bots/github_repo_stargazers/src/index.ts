import { log, query, addEntry } from "./library";

export function meta() {
  const data = {
    name: "github_repo_stargazers",
    version: "0.0.1",
  };

  Host.outputString(JSON.stringify(data));
}

export function main() {
  const schema = "7vtxfvpypm2ha7c5hpmy3t2e26glim256ebphxxfar6jqrzzwpya";
  log("sup");
  
  const phasers = Config.get("phasers")
  if (!phasers) {
    return -1;
  }

  const req: HttpRequest = {
    url: `https://postman-echo.com/get?phasers=${phasers}`,
    method: "GET",
  };
  let res = Http.request(req);
  if (res.status !== 200) {
    return -2;
  }

  let body = JSON.parse(res.body);
  const added = addEntry(schema, body);
  log(JSON.stringify(added));

  let result = query(schema, "hello");
  log(JSON.stringify(result));

  log("done");
}
