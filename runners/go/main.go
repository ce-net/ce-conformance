// ce-conformance Go runner — drives the ce-go SDK through the shared Tier-A scenario
// contract (see ../../SCENARIOS.md) against the live node at $CE_NODE_URL, using ONLY the
// SDK's public API. It emits one machine-readable line per scenario:
//
//	CONF <scenario_id> PASS
//	CONF <scenario_id> FAIL: <detail>
//
// and exits non-zero if any scenario fails. Every language runner implements the same
// scenarios with the same ids and the same output contract, so the driver (../../run.sh)
// can build one cross-language matrix. Adding a language = writing one of these.
package main

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"time"

	ce "github.com/ce-net/ce-go"
)

type result struct {
	id     string
	pass   bool
	detail string
}

// nonce yields a per-run-unique suffix so a scenario's topic never collides with another
// run or with unrelated background mesh traffic on the shared node.
func nonce() string { return fmt.Sprintf("%d-%d", os.Getpid(), time.Now().UnixNano()) }

func main() {
	ctx := context.Background()
	c := ce.Connect()
	if err := c.WaitReady(ctx, 15*time.Second); err != nil {
		fmt.Printf("CONF setup FAIL: node not ready: %v\n", err)
		os.Exit(2)
	}
	self, err := c.NodeID(ctx)
	if err != nil || self == "" {
		fmt.Printf("CONF setup FAIL: no node id: %v\n", err)
		os.Exit(2)
	}

	results := []result{
		s1Status(ctx, c),
		s2PubsubText(ctx, c),
		s3BinaryPayload(ctx, c),
		s4RequestReply(ctx, c, self),
		s5RequestUnknownErrors(ctx, c),
	}

	allPass := true
	for _, r := range results {
		if r.pass {
			fmt.Printf("CONF %s PASS\n", r.id)
		} else {
			allPass = false
			fmt.Printf("CONF %s FAIL: %s\n", r.id, r.detail)
		}
	}
	if !allPass {
		os.Exit(1)
	}
}

// s1: the node answers /status with a non-empty node id (liveness + shape).
func s1Status(ctx context.Context, c *ce.Client) result {
	s, err := c.Status(ctx)
	if err != nil {
		return result{"status", false, err.Error()}
	}
	if s.NodeID == "" {
		return result{"status", false, "empty node_id"}
	}
	return result{"status", true, ""}
}

// awaitPublish subscribes to a fresh topic, publishes payload, and returns the first payload
// received on that topic (bounded). This exercises subscribe + publish + the SSE inbound stream.
func awaitPublish(ctx context.Context, c *ce.Client, topic string, payload []byte) ([]byte, error) {
	sctx, cancel := context.WithCancel(ctx)
	defer cancel()
	msgs, err := c.Subscribe(sctx, topic)
	if err != nil {
		return nil, err
	}
	time.Sleep(600 * time.Millisecond) // let the subscribe register and the stream establish
	if err := c.Publish(ctx, topic, payload); err != nil {
		return nil, err
	}
	select {
	case m := <-msgs:
		return m.Payload, nil
	case <-time.After(8 * time.Second):
		return nil, fmt.Errorf("timeout: no message on %s", topic)
	}
}

// s2: a UTF-8 publish round-trips to a subscriber, payload intact.
func s2PubsubText(ctx context.Context, c *ce.Client) result {
	topic := "conf/pubsub-text/" + nonce()
	want := []byte("hello-conformance")
	got, err := awaitPublish(ctx, c, topic, want)
	if err != nil {
		return result{"pubsub_text", false, err.Error()}
	}
	if !bytes.Equal(got, want) {
		return result{"pubsub_text", false, fmt.Sprintf("got %q want %q", got, want)}
	}
	return result{"pubsub_text", true, ""}
}

// s3: a non-UTF-8 binary payload round-trips byte-exact (proves the hex wire, not text mangling).
func s3BinaryPayload(ctx context.Context, c *ce.Client) result {
	topic := "conf/pubsub-bin/" + nonce()
	want := []byte{0x00, 0xff, 0x10, 0x7f, 0x00, 0xab}
	got, err := awaitPublish(ctx, c, topic, want)
	if err != nil {
		return result{"binary_payload", false, err.Error()}
	}
	if !bytes.Equal(got, want) {
		return result{"binary_payload", false, fmt.Sprintf("got %v want %v", got, want)}
	}
	return result{"binary_payload", true, ""}
}

// s4: a Serve responder answers a directed Request end to end (the request/reply loop).
func s4RequestReply(ctx context.Context, c *ce.Client, self string) result {
	topic := "conf/reqrep/" + nonce()
	sctx, cancel := context.WithCancel(ctx)
	defer cancel()
	go c.Serve(sctx, []string{topic}, func(m ce.Message) ([]byte, error) {
		return append([]byte("echo:"), m.Payload...), nil
	})
	time.Sleep(600 * time.Millisecond) // let the responder subscribe
	out, err := c.Request(ctx, self, topic, []byte("ping"), 8*time.Second)
	if err != nil {
		return result{"request_reply", false, err.Error()}
	}
	if string(out) != "echo:ping" {
		return result{"request_reply", false, fmt.Sprintf("got %q", out)}
	}
	return result{"request_reply", true, ""}
}

// s5: a request to an unreachable node id errors within its timeout — it neither hangs nor
// reports false success. Proves error propagation and timeout bounding.
func s5RequestUnknownErrors(ctx context.Context, c *ce.Client) result {
	bogus := "0000000000000000000000000000000000000000000000000000000000000000"
	start := time.Now()
	_, err := c.Request(ctx, bogus, "conf/nonexistent/"+nonce(), []byte("x"), 3*time.Second)
	elapsed := time.Since(start)
	if err == nil {
		return result{"request_unknown_errors", false, "expected error, got success"}
	}
	if elapsed > 9*time.Second {
		return result{"request_unknown_errors", false, fmt.Sprintf("did not bound: took %v", elapsed)}
	}
	return result{"request_unknown_errors", true, ""}
}
