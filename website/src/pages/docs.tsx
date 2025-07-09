import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { Image } from "@heroui/image";
import { Link } from "@heroui/link";
import { Snippet } from "@heroui/snippet";

export default function DocsPage() {
  return (
    <DefaultLayout>
      <main className="w-full p-8">
        <section id="why" className="mb-16 scroll-mt-24">
          <div className="space-y-4">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>Why?</h2>
            <p className="text-default-700">
              You can't be serious...another web performance testing tool?
              You're right to roll your eyes, but give us a chance to justify
              why we made whambam.
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <span>$ brew doctor</span>
              <span>
                Warning: Some installed formulae are deprecated or disabled.
              </span>
              <span>
                You should find replacements for the following formulae:
              </span>
              <span>&nbsp;&nbsp;&nbsp;&nbsp;hey</span>
              <span>$ brew info hey</span>
              <span>==&gt; hey: stable 0.1.4 (bottled)</span>
              <span>HTTP load generator, ApacheBench (ab) replacement</span>
              <span>https://github.com/rakyll/hey</span>
              <span>
                Deprecated because it is not maintained upstream! It will be
                disabled on 2026-01-12.
              </span>
            </Snippet>
            <p className="text-default-700">
              Wait, what? But we love{" "}
              <Link href="https://github.com/rakyll/hey" isExternal>
                hey
              </Link>
              ! That's too bad.
            </p>
            <p className="text-default-700">
              We looked around and there are more web testing tools than you can
              imagine. But it's hard to find a tool that's a good replacement
              for hey, which has the ideal mix of speed, simplicity, and
              features.
            </p>
            <p className="text-default-700">
              Although one of the things we liked looking at alternatives was a
              way to see progress. We're not signing up for a full UI, but can
              we do something simpler and still have it feel like a terminal
              application?
            </p>
            <p className="text-default-700">
              We gave it a shot and we hope you like what we came up with.
            </p>
            <p className="text-default-700">
              Here's our design and implementation focus.
              <ul className="list-disc pl-5">
                <li>
                  As fast as <code>hey</code>
                </li>
                <li>
                  Command-line argument compatibility with <code>hey</code>
                </li>
                <li>A simple progress UI</li>
                <li>A cleanroom implementation</li>
              </ul>
            </p>
          </div>
        </section>

        <section id="install" className="mb-16 scroll-mt-24">
          <div className="space-y-4">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              Install
            </h2>
            <p className="text-default-700">
              If you're on a Mac and using homebrew, just do this.
            </p>
            <Snippet variant="bordered">
              <span>brew tap cloudy-native/whambam</span>
              <span>brew install whambam</span>
            </Snippet>
            <p className="text-default-700">
              If you're not using homebrew, this is your chance and it will
              definitely be worth your while. Follow instructions at{" "}
              <Link href="https://brew.sh" isExternal>
                brew.sh
              </Link>
              .
            </p>
            <p className="text-default-700">
              We're planning support for Linux and Windows soon.
            </p>
          </div>
        </section>

        <section id="usage" className="mb-16 scroll-mt-24">
          <div className="space-y-6">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>Usage</h2>
            <p className="text-default-700">
              Get started with whambam by pointing it at a URL and letting good
              defaults do the rest.
            </p>
            <Snippet hideCopyButton variant="bordered">
              <span>whambam -z 10s https://example.com</span>
            </Snippet>
            <p className="text-default-700">
              Will pummel <code>https://example.com</code> for 10 seconds with
              the default 50 concurrent connections.
            </p>

            <h3 className="text-xl font-bold pt-4">Core Options</h3>
            <div className="overflow-x-auto">
              <table className="min-w-full divide-y divide-default-200">
                <thead className="bg-default-100">
                  <tr>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Option
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Description
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Default
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Examples & Explanation
                    </th>
                  </tr>
                </thead>
                <tbody className="bg-background divide-y divide-default-200">
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -n, --requests &lt;N&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Number of requests to send
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      200
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-n 1000</code> sends exactly 1000 requests. The test
                      ends when all requests are complete. Cannot be used with{" "}
                      <code>-z</code>.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -c, --concurrent &lt;N&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Concurrent connections
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      50
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-c 100</code> simulates 100 users making requests
                      simultaneously.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -z, --duration &lt;TIME&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Test duration (e.g., 30s, 5m, 1h)
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      unlimited
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-z 1m</code> runs the test for exactly 1 minute.
                      Cannot be used with <code>-n</code>.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -t, --timeout &lt;SEC&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Request timeout in seconds
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      20
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-t 5</code> aborts any request that takes longer
                      than 5 seconds.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -q, --rate-limit &lt;QPS&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Rate limit (queries per second)
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      unlimited
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-q 100</code> attempts to send 100 requests per
                      second. If <code>-c</code> is too low, the actual rate may
                      be lower.
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            <h3 className="text-xl font-bold pt-4">HTTP Configuration</h3>
            <div className="overflow-x-auto">
              <table className="min-w-full divide-y divide-default-200">
                <thead className="bg-default-100">
                  <tr>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Option
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Description
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Default
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Examples & Explanation
                    </th>
                  </tr>
                </thead>
                <tbody className="bg-background divide-y divide-default-200">
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -m, --method &lt;METHOD&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      HTTP method (<code>GET</code>, <code>POST</code>,{" "}
                      <code>PUT</code>, <code>DELETE</code>, <code>HEAD</code>,{" "}
                      <code>OPTIONS</code>)
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      GET
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-m POST</code> sends a POST request. Usually used
                      with <code>-d</code> or <code>-D</code>.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -d, --body &lt;BODY&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Request body
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      -
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>{`-d '{"key":"value"}'`}</code> sends the given JSON
                      string as the request body.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -D, --body-file &lt;FILE&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Request body from file
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      -
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-D /path/to/body.json</code> sends the contents of
                      the file as the request body.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -H, --header &lt;HEADER&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Custom headers (repeatable)
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      -
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>
                        -H 'X-My-Header: 123' -H 'User-Agent: whambam'
                      </code>
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -A, --accept &lt;HEADER&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Accept header
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      -
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-A 'application/json'</code> sets the Accept header.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -T, --content-type &lt;TYPE&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Content-Type header
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      text/html
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-T 'application/json'</code>. Important when sending
                      a request body.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -a, --auth &lt;USER:PASS&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Basic authentication
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-default-600">
                      -
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-a admin:s3cr3t</code> sends an Authorization header
                      with the credentials.
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            <h3 className="text-xl font-bold pt-4">Network Options</h3>
            <div className="overflow-x-auto">
              <table className="min-w-full divide-y divide-default-200">
                <thead className="bg-default-100">
                  <tr>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Option
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Description
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Examples & Explanation
                    </th>
                  </tr>
                </thead>
                <tbody className="bg-background divide-y divide-default-200">
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -x, --proxy &lt;HOST:PORT&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      HTTP proxy
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code>-x http://127.0.0.1:8080</code> routes all requests
                      through the specified proxy.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      --disable-compression
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Disable compression
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Prevents whambam from requesting compressed responses
                      (e.g., gzip).
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      --disable-keepalive
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Disable connection reuse
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Forces a new TCP connection for each request. Simulates
                      clients that don't support keep-alive.
                    </td>
                  </tr>
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      --disable-redirects
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Disable redirect following
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      If the server returns a 3xx redirect, whambam will not
                      follow it.
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            <h3 className="text-xl font-bold pt-4">Output Options</h3>
            <div className="overflow-x-auto">
              <table className="min-w-full divide-y divide-default-200">
                <thead className="bg-default-100">
                  <tr>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Option
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Description
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Default
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-default-500 uppercase tracking-wider">
                      Examples & Explanation
                    </th>
                  </tr>
                </thead>
                <tbody className="bg-background divide-y divide-default-200">
                  <tr>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-sm text-default-800">
                      -o, --output &lt;FORMAT&gt;
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      Output format:{" "}
                      <code className="font-mono bg-default-100 p-1 rounded">
                        ui
                      </code>{" "}
                      for a simple terminal UI or{" "}
                      <code className="font-mono bg-default-100 p-1 rounded">
                        hey
                      </code>{" "}
                      for mostly hey-compatible text output
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      <code className="font-mono bg-default-100 p-1 rounded">
                        ui
                      </code>
                    </td>
                    <td className="px-6 py-4 whitespace-normal text-sm text-default-600">
                      NOTE: temporarily disabled in current version.{" "}
                      <code>-o hey</code> is useful for scripting or logging, as
                      it prints a simple text summary.
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            <h3 className="text-xl font-bold pt-4">Interactive UI Guide</h3>
            <p className="text-default-700">
              The interactive UI provides real-time feedback on your load test.
              Use these keys to navigate:
            </p>
            <h4 className="text-lg font-semibold pt-2">Navigation</h4>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>
                <code className="font-mono bg-default-100 p-1 rounded">1</code>,{" "}
                <code className="font-mono bg-default-100 p-1 rounded">2</code>,{" "}
                <code className="font-mono bg-default-100 p-1 rounded">3</code>:
                Switch between Dashboard, Charts, and Status Codes tabs
              </li>
              <li>
                <code className="font-mono bg-default-100 p-1 rounded">h</code>{" "}
                or{" "}
                <code className="font-mono bg-default-100 p-1 rounded">?</code>:
                Toggle help overlay
              </li>
              <li>
                <code className="font-mono bg-default-100 p-1 rounded">
                  Ctrl-C
                </code>
                ,{" "}
                <code className="font-mono bg-default-100 p-1 rounded">q</code>,
                or{" "}
                <code className="font-mono bg-default-100 p-1 rounded">
                  ESC
                </code>
                : Exit application
              </li>
            </ul>

            <h4 className="text-lg font-semibold pt-2">Dashboard Tab</h4>
            <Image
              shadow="sm"
              radius="lg"
              width="100%"
              alt="Dasboard tab"
              src="/images/ui-tab-1.png"
            />
            <p className="text-default-700">
              Real-time performance metrics including:
            </p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>
                <strong>Throughput</strong>: Requests per second
              </li>
              <li>
                <strong>Success Rate</strong>: Percentage of successful requests
              </li>
              <li>
                <strong>Response Times</strong>: Min, max, and average latency
              </li>
              <li>
                <strong>Live Charts</strong>: Visual representation of
                performance trends
              </li>
            </ul>

            <h4 className="text-lg font-semibold pt-2">Charts Tab</h4>
            <Image
              shadow="sm"
              radius="lg"
              width="100%"
              alt="Dasboard tab"
              src="/images/ui-tab-2.png"
            />
            <p className="text-default-700">Full-screen visualization of:</p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>Throughput over time</li>
              <li>Latency distribution</li>
              <li>Request completion trends</li>
            </ul>

            <h4 className="text-lg font-semibold pt-2">Status Codes Tab</h4>
            <Image
              shadow="sm"
              radius="lg"
              width="100%"
              alt="Dasboard tab"
              src="/images/ui-tab-3.png"
            />
            <p className="text-default-700">
              Detailed breakdown of HTTP responses:
            </p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>Color-coded by status class (2xx, 3xx, 4xx, 5xx)</li>
              <li>Percentage distribution</li>
              <li>Real-time updates</li>
            </ul>
          </div>
        </section>
      </main>
    </DefaultLayout>
  );
}
