#include <napi/native_api.h>
#include <string>

extern "C" char *hatoba_native_probe();
extern "C" void hatoba_string_free(char *value);

namespace {
struct ProbeWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string result;
};

void Execute(napi_env, void *data)
{
    auto *work = static_cast<ProbeWork *>(data);
    char *result = hatoba_native_probe();
    if (result != nullptr) {
        work->result = result;
        hatoba_string_free(result);
    }
}

void Complete(napi_env env, napi_status status, void *data)
{
    auto *work = static_cast<ProbeWork *>(data);
    napi_value value;
    if (status == napi_ok && !work->result.empty() &&
        napi_create_string_utf8(env, work->result.c_str(), work->result.size(), &value) == napi_ok) {
        napi_resolve_deferred(env, work->deferred, value);
    } else {
        napi_value message;
        napi_create_string_utf8(env, "Native probe failed", NAPI_AUTO_LENGTH, &message);
        napi_create_error(env, nullptr, message, &value);
        napi_reject_deferred(env, work->deferred, value);
    }
    napi_delete_async_work(env, work->work);
    delete work;
}

napi_value Probe(napi_env env, napi_callback_info)
{
    auto *work = new ProbeWork;
    napi_value promise;
    napi_value name;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok ||
        napi_create_string_utf8(env, "HatobaNativeProbe", NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_async_work(env, nullptr, name, Execute, Complete, work, &work->work) != napi_ok) {
        delete work;
        napi_throw_error(env, nullptr, "Could not initialize native probe");
        return nullptr;
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_value message;
        napi_value error;
        napi_create_string_utf8(env, "Could not queue native probe", NAPI_AUTO_LENGTH, &message);
        napi_create_error(env, nullptr, message, &error);
        napi_reject_deferred(env, work->deferred, error);
        napi_delete_async_work(env, work->work);
        delete work;
    }
    return promise;
}

napi_value Init(napi_env env, napi_value exports)
{
    napi_property_descriptor properties[] = {
        {"probe", nullptr, Probe, nullptr, nullptr, nullptr, napi_default, nullptr},
    };
    napi_define_properties(env, exports, 1, properties);
    return exports;
}

napi_module module = {1, 0, nullptr, Init, "hatoba", nullptr, {nullptr}};
}

extern "C" __attribute__((constructor)) void RegisterHatobaModule()
{
    napi_module_register(&module);
}
