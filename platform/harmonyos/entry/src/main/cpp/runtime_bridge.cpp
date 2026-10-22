#include <napi/native_api.h>
#include <cstdint>
#include <cstring>
#include <string>

struct NativePacket { uint32_t kind; uint8_t *data; size_t size; };
extern "C" char *hatoba_initialize(const char *config, uint64_t epoch);
extern "C" uint64_t hatoba_page_epoch();
extern "C" char *hatoba_submit(const char *message);
extern "C" char *hatoba_system_reply(const char *message);
extern "C" char *hatoba_lifecycle(const char *event);
extern "C" char *hatoba_grant(int32_t fd, const char *name, bool write);
extern "C" NativePacket *hatoba_poll(bool system);
extern "C" void hatoba_packet_free(NativePacket *packet);
extern "C" void hatoba_detach();
extern "C" void hatoba_string_free(char *value);

namespace {
bool String(napi_env env, napi_value value, std::string &out)
{
    size_t length = 0;
    if (napi_get_value_string_utf8(env, value, nullptr, 0, &length) != napi_ok || length > 4 * 1024 * 1024) {
        napi_throw_type_error(env, nullptr, "Invalid native string argument"); return false;
    }
    out.resize(length);
    return napi_get_value_string_utf8(env, value, out.data(), out.size() + 1, &length) == napi_ok;
}
napi_value Json(napi_env env, char *json)
{
    if (json == nullptr) { napi_throw_error(env, nullptr, "Native operation failed"); return nullptr; }
    napi_value value;
    const napi_status status = napi_create_string_utf8(env, json, NAPI_AUTO_LENGTH, &value);
    hatoba_string_free(json);
    if (status != napi_ok) { napi_throw_error(env, nullptr, "Could not encode native result"); return nullptr; }
    return value;
}
struct InitWork { napi_async_work work = nullptr; napi_deferred deferred = nullptr; std::string config; uint64_t epoch = 0; char *result = nullptr; };
void InitializeExecute(napi_env, void *data) { auto *work = static_cast<InitWork *>(data); work->result = hatoba_initialize(work->config.c_str(), work->epoch); }
void InitializeComplete(napi_env env, napi_status status, void *data)
{
    auto *work = static_cast<InitWork *>(data);
    if (status == napi_ok && work->result != nullptr) {
        napi_value value = Json(env, work->result); work->result = nullptr;
        if (value != nullptr) napi_resolve_deferred(env, work->deferred, value);
    } else {
        napi_value message, error;
        napi_create_string_utf8(env, "Native initialization failed", NAPI_AUTO_LENGTH, &message);
        napi_create_error(env, nullptr, message, &error); napi_reject_deferred(env, work->deferred, error);
    }
    if (work->result != nullptr) hatoba_string_free(work->result);
    napi_delete_async_work(env, work->work); delete work;
}
napi_value Initialize(napi_env env, napi_callback_info info)
{
    napi_value args[1]; size_t count = 1;
    napi_get_cb_info(env, info, &count, args, nullptr, nullptr);
    auto *work = new InitWork;
    work->epoch = hatoba_page_epoch();
    if (count != 1 || !String(env, args[0], work->config)) { delete work; return nullptr; }
    napi_value promise, name;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok ||
        napi_create_string_utf8(env, "HatobaInitialize", NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_async_work(env, nullptr, name, InitializeExecute, InitializeComplete, work, &work->work) != napi_ok) {
        delete work; napi_throw_error(env, nullptr, "Could not create native initialization"); return nullptr;
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work); delete work;
        napi_throw_error(env, nullptr, "Could not queue native initialization"); return nullptr;
    }
    return promise;
}
napi_value Control(napi_env env, napi_callback_info info)
{
    napi_value args[1]; size_t count = 1; void *data = nullptr;
    napi_get_cb_info(env, info, &count, args, nullptr, &data);
    std::string input;
    if (count != 1 || !String(env, args[0], input)) return nullptr;
    char *result = data == nullptr ? hatoba_submit(input.c_str()) :
        *static_cast<int *>(data) == 2 ? hatoba_lifecycle(input.c_str()) : hatoba_system_reply(input.c_str());
    volatile char *bytes = input.data(); for (size_t i = 0; i < input.size(); ++i) bytes[i] = 0;
    return Json(env, result);
}
napi_value Grant(napi_env env, napi_callback_info info)
{
    napi_value args[3]; size_t count = 3; int32_t fd = -1; bool write = false; std::string name;
    napi_get_cb_info(env, info, &count, args, nullptr, nullptr);
    if (count != 3 || napi_get_value_int32(env, args[0], &fd) != napi_ok ||
        !String(env, args[1], name) || napi_get_value_bool(env, args[2], &write) != napi_ok) {
        napi_throw_type_error(env, nullptr, "Invalid document grant"); return nullptr;
    }
    return Json(env, hatoba_grant(fd, name.c_str(), write));
}
napi_value Drain(napi_env env, napi_callback_info info)
{
    size_t count = 0; void *data = nullptr;
    napi_get_cb_info(env, info, &count, nullptr, nullptr, &data);
    napi_value array; napi_create_array(env, &array);
    for (uint32_t index = 0; index < 64; ++index) {
        NativePacket *packet = hatoba_poll(data != nullptr); if (packet == nullptr) break;
        napi_value value; napi_status status = napi_invalid_arg;
        if (packet->kind == 1) status = napi_create_string_utf8(env, reinterpret_cast<char *>(packet->data), packet->size, &value);
        else if (packet->kind == 2) {
            void *buffer = nullptr;
            status = napi_create_arraybuffer(env, packet->size, &buffer, &value);
            if (status == napi_ok) std::memcpy(buffer, packet->data, packet->size);
        }
        hatoba_packet_free(packet);
        if (status != napi_ok) { napi_throw_error(env, nullptr, "Could not receive native message"); return nullptr; }
        napi_set_element(env, array, index, value);
    }
    return array;
}
napi_value Detach(napi_env env, napi_callback_info)
{
    hatoba_detach(); napi_value value; napi_get_undefined(env, &value); return value;
}
}
void DefineRuntime(napi_env env, napi_value exports)
{
    static int system = 1;
    static int lifecycle = 2;
    napi_property_descriptor properties[] = {
        {"initialize", nullptr, Initialize, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"submit", nullptr, Control, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"systemReply", nullptr, Control, nullptr, nullptr, nullptr, napi_default, &system},
        {"lifecycle", nullptr, Control, nullptr, nullptr, nullptr, napi_default, &lifecycle},
        {"grant", nullptr, Grant, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"drain", nullptr, Drain, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"drainSystem", nullptr, Drain, nullptr, nullptr, nullptr, napi_default, &system},
        {"detach", nullptr, Detach, nullptr, nullptr, nullptr, napi_default, nullptr},
    };
    napi_define_properties(env, exports, sizeof(properties) / sizeof(properties[0]), properties);
}
