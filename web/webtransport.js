// Playing on a server from the browser: the game's WebTransport connections (its renet2 client
// socket calls these). A server's certificate is a self-signed one browsers take by its hash,
// which the server tells over plain HTTP on the same port.
miniquad_add_plugin({
    name: "soldank_net",
    version: 1,
    register_plugin: function (importObject) {
        var connections = [];
        var CONNECTING = 0, OPEN = 1, CLOSED = 2;

        function text(ptr, len) {
            return new TextDecoder().decode(new Uint8Array(wasm_memory.buffer, ptr, len));
        }

        function hexBytes(hex) {
            var bytes = new Uint8Array(hex.length / 2);
            for (var i = 0; i < bytes.length; i++) {
                bytes[i] = parseInt(hex.substr(2 * i, 2), 16);
            }
            return bytes;
        }

        // opens `url`, after asking `info` for the certificate's hash (without one the browser
        // checks the certificate as usual); the connection's number
        importObject.env.soldank_wt_open = function (url_ptr, url_len, info_ptr, info_len) {
            var url = text(url_ptr, url_len);
            var info = text(info_ptr, info_len);
            var connection = { state: CONNECTING, incoming: [], writer: null, transport: null };
            var id = connections.length;
            connections.push(connection);
            if (typeof WebTransport === "undefined") {
                console.warn("this browser has no WebTransport (or the page isn't served securely)");
                connection.state = CLOSED;
                return id;
            }
            var closed = function () {
                connection.state = CLOSED;
            };
            fetch(info)
                .then(function (response) {
                    return response.json();
                })
                .then(
                    function (answer) {
                        return answer.hash;
                    },
                    function () {
                        return null;
                    }
                )
                .then(function (hash) {
                    if (connection.state === CLOSED) {
                        return;
                    }
                    var options = { requireUnreliable: true, congestionControl: "low-latency" };
                    if (hash) {
                        options.serverCertificateHashes = [{ algorithm: "sha-256", value: hexBytes(hash) }];
                    }
                    var transport = new WebTransport(url, options);
                    connection.transport = transport;
                    transport.closed.then(closed, closed);
                    return transport.ready.then(function () {
                        connection.writer = transport.datagrams.writable.getWriter();
                        connection.state = OPEN;
                        var reader = transport.datagrams.readable.getReader();
                        var read = function () {
                            reader.read().then(function (result) {
                                if (result.done) {
                                    closed();
                                    return;
                                }
                                connection.incoming.push(result.value);
                                read();
                            }, closed);
                        };
                        read();
                    });
                })
                .catch(function (error) {
                    console.warn("WebTransport:", error);
                    closed();
                });
            return id;
        };

        importObject.env.soldank_wt_state = function (id) {
            var connection = connections[id];
            return connection ? connection.state : CLOSED;
        };

        // a datagram (dropped until the connection is open)
        importObject.env.soldank_wt_send = function (id, ptr, len) {
            var connection = connections[id];
            if (connection && connection.state === OPEN && connection.writer) {
                var datagram = new Uint8Array(wasm_memory.buffer, ptr, len).slice();
                connection.writer.write(datagram).catch(function () {});
            }
        };

        // the next datagram into the game's memory: its length, -1 without one
        importObject.env.soldank_wt_recv = function (id, ptr, cap) {
            var connection = connections[id];
            if (!connection || connection.incoming.length === 0) {
                return -1;
            }
            var datagram = connection.incoming.shift();
            if (datagram.length > cap) {
                return -2;
            }
            new Uint8Array(wasm_memory.buffer, ptr, datagram.length).set(datagram);
            return datagram.length;
        };

        importObject.env.soldank_wt_close = function (id) {
            var connection = connections[id];
            if (!connection) {
                return;
            }
            connection.state = CLOSED;
            if (connection.transport) {
                try {
                    connection.transport.close();
                } catch (error) {}
            }
            connections[id] = null;
        };
    },
});
