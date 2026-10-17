#include "../Redirector/SocksHelper.h"
#include <stdexcept>
#include <cstring>

wstring tgtHost = L"127.0.0.1";
wstring tgtPort;
string tgtUsername;
string tgtPassword;

static void Check(bool value, const char* message)
{
    if (!value) throw runtime_error(message);
}

struct SocketPair
{
    SOCKET client = INVALID_SOCKET, server = INVALID_SOCKET;
    SocketPair()
    {
        SOCKET listener = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
        Check(listener != INVALID_SOCKET, "listener");
        SOCKADDR_IN address{};
        address.sin_family = AF_INET;
        address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        Check(bind(listener, (PSOCKADDR)&address, sizeof(address)) == 0, "bind");
        int size = sizeof(address);
        Check(getsockname(listener, (PSOCKADDR)&address, &size) == 0, "getsockname");
        Check(listen(listener, 1) == 0, "listen");
        client = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
        Check(connect(client, (PSOCKADDR)&address, sizeof(address)) == 0, "connect");
        server = accept(listener, nullptr, nullptr);
        closesocket(listener);
        Check(server != INVALID_SOCKET, "accept");
        DWORD timeout = 3000;
        setsockopt(server, SOL_SOCKET, SO_RCVTIMEO, (char*)&timeout, sizeof(timeout));
        setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, (char*)&timeout, sizeof(timeout));
    }
    ~SocketPair()
    {
        if (client != INVALID_SOCKET) closesocket(client);
        if (server != INVALID_SOCKET) closesocket(server);
    }
};

static void TestUDPBounds()
{
    for (int type : {1, 4})
    {
        const int header = type == 1 ? 10 : 22;
        for (int length = 0; length < header; ++length)
        {
            vector<char> data(length, 0);
            if (length >= 4) data[3] = (char)type;
            Check(SocksHelper::DecodeUDP(nullptr, data.data(), length) == SOCKET_ERROR, "short UDP packet accepted");
        }
        vector<char> packet(header + 4, 0);
        packet[3] = (char)type;
        memcpy(packet.data() + header, "test", 4);
        SOCKADDR_IN6 target{};
        Check(SocksHelper::DecodeUDP(&target, packet.data(), (int)packet.size()) == 4, "valid UDP rejected");
        Check(memcmp(packet.data(), "test", 4) == 0, "UDP payload damaged");
        Check(target.sin6_family == (type == 1 ? AF_INET : AF_INET6), "UDP address family");
    }
    char fragmented[22]{};
    fragmented[2] = 1; fragmented[3] = 4;
    Check(SocksHelper::DecodeUDP(nullptr, fragmented, 22) == SOCKET_ERROR, "fragment accepted");
    fragmented[2] = 0; fragmented[0] = 1;
    Check(SocksHelper::DecodeUDP(nullptr, fragmented, 22) == SOCKET_ERROR, "reserved byte accepted");
}

static void TestFragmentedHandshake()
{
    SocketPair pair;
    bool peerOK = true;
    thread peer([&] {
        char hello[4]{};
        peerOK = SocksHelper::ReceiveExact(pair.server, hello, 4);
        const char reply[] = {5, 0, 1, 127, 0, 0, 1, 0, 80};
        for (char value : reply)
        {
            peerOK = peerOK && SocksHelper::SendAll(pair.server, &value, 1);
            this_thread::sleep_for(chrono::milliseconds(10));
        }
    });
    const bool handshake = SocksHelper::Handshake(pair.client);
    SOCKADDR_IN6 address{};
    const bool parsed = handshake && SocksHelper::SplitAddr(pair.client, &address);
    peer.join();
    Check(peerOK && parsed && address.sin6_family == AF_INET, "fragmented handshake failed");
}

static void TestControlThreadOwnership()
{
    SocketPair pair;
    SocksHelper::UDP* udp = new SocksHelper::UDP();
    udp->tcpSocket = pair.client;
    pair.client = INVALID_SOCKET;
    udp->address.sin6_family = AF_INET;
    Check(udp->CreateUDP(), "create UDP");
    // Destruction must wake and join a blocked control reader, without double-close.
    const auto started = chrono::steady_clock::now();
    delete udp;
    Check(chrono::steady_clock::now() - started < chrono::seconds(2), "control reader did not stop");
}

int main(int argc, char** argv)
{
    if (argc > 1 && strcmp(argv[1], "--guard-output") == 0)
    {
        thread standard([] { for (int i = 0; i < 1000; ++i) printf("stdout-%d\n", i); fflush(stdout); });
        thread errors([] { for (int i = 0; i < 1000; ++i) fprintf(stderr, "stderr-%d\n", i); fflush(stderr); });
        standard.join(); errors.join();
        return 0;
    }
    WSADATA data{};
    if (WSAStartup(MAKEWORD(2, 2), &data) != 0) return 1;
    try
    {
        TestUDPBounds();
        TestFragmentedHandshake();
        for (int i = 0; i < 20; ++i) TestControlThreadOwnership();
        puts("Native SOCKS regression tests passed (UDP bounds, fragmented handshake, 20 shutdown cycles).");
    }
    catch (const exception& e) { fprintf(stderr, "%s\n", e.what()); WSACleanup(); return 1; }
    WSACleanup();
    return 0;
}
