#!/usr/bin/env python3
"""Generate checked-in native projects using only Python's standard library.
Run from any directory: python3 apple-shared/generate-projects.py.
No dependency installation, Xcode build, formatting or tests are performed.
"""
from pathlib import Path
import hashlib
import json
import plistlib

ROOT = Path(__file__).resolve().parent.parent

def quote(s):
    return json.dumps(str(s))

def identifier(s):
    return hashlib.sha256(s.encode()).hexdigest()[:24].upper()

def project(platform):
    folder = ROOT / platform
    main = 'HomeHubMac' if platform == 'macos' else 'HomeHub'
    targets = [main] + (['HomeHubShare'] if platform == 'ios' else []) + [main + 'Tests']
    objects = {}
    def add(key, kind, **values):
        uid = identifier(platform + ':' + key)
        objects[uid] = dict(isa=kind, **values)
        return uid
    def raw(uid): return ('raw', uid)
    def serialize(v):
        if isinstance(v, tuple): return v[1]
        if isinstance(v, dict): return '{ ' + ' '.join(f'{quote(k)} = {serialize(val)};' for k,val in v.items()) + ' }'
        if isinstance(v, list): return '(' + ', '.join(serialize(x) for x in v) + ')'
        return quote(v)
    refs = {}
    def file(path, kind=None):
        if path not in refs:
            if kind is None: kind = 'sourcecode.swift' if path.endswith('.swift') else 'text.html'
            refs[path] = add('file:' + path, 'PBXFileReference', path=path, sourceTree='<group>', lastKnownFileType=kind)
        return refs[path]
    product_refs = {}
    for target in targets:
        is_test = target.endswith('Tests'); is_share = target == 'HomeHubShare'
        ext = '.xctest' if is_test else '.appex' if is_share else '.app'
        kind = 'wrapper.cfbundle' if is_test else 'wrapper.app-extension' if is_share else 'wrapper.application'
        product_refs[target] = add('product:' + target, 'PBXFileReference', path=target+ext, sourceTree='BUILT_PRODUCTS_DIR', explicitFileType=kind, includeInIndex='0')
    localization = {}
    for target in targets:
        if target.endswith('Tests'): continue
        children = []
        for language in ['en','hi']:
            children.append(raw(add('locale:'+target+language, 'PBXFileReference', name=language,
                path=f'{target}/Resources/{language}.lproj/Localizable.strings', sourceTree='<group>', lastKnownFileType='text.plist.strings')))
        localization[target] = add('strings:'+target, 'PBXVariantGroup', name='Localizable.strings', sourceTree='<group>', children=children)
    target_ids = {t: identifier(platform + ':target:' + t) for t in targets}
    project_id = identifier(platform + ':project')
    for target in targets:
        test = target.endswith('Tests'); share = target == 'HomeHubShare'
        if test: paths = ['../apple-shared/Tests/HomeHubTests.swift']
        else:
            paths = sorted(str(p.relative_to(folder)) for p in (folder/target).glob('*.swift'))
            shared = ['Protocol.swift','Blake3.swift','QueueStore.swift'] if share else ['Protocol.swift','Blake3.swift','QueueStore.swift','UploadEngine.swift','Identity.swift','HubClient.swift','Discovery.swift']
            paths += ['../apple-shared/'+p for p in shared]
        sources = [raw(add('build:'+target+path,'PBXBuildFile',fileRef=raw(file(path)))) for path in paths]
        source_phase = add('sources:'+target,'PBXSourcesBuildPhase',buildActionMask='2147483647',files=sources,runOnlyForDeploymentPostprocessing='0')
        frameworks = add('frameworks:'+target,'PBXFrameworksBuildPhase',buildActionMask='2147483647',files=[],runOnlyForDeploymentPostprocessing='0')
        resource_files = [] if test else [raw(add('build:strings:'+target,'PBXBuildFile',fileRef=raw(localization[target])))]
        if platform == 'macos' and not test:
            resource_files += [raw(add('build:viewer','PBXBuildFile',fileRef=raw(file('HomeHubMac/Resources/viewer.html'))))]
        resources = add('resources:'+target,'PBXResourcesBuildPhase',buildActionMask='2147483647',files=resource_files,runOnlyForDeploymentPostprocessing='0')
        phases = [raw(source_phase),raw(frameworks),raw(resources)]
        dependencies = []
        if test or (platform == 'ios' and target == main):
            dependency_target = main if test else 'HomeHubShare'
            proxy = add('proxy:'+target,'PBXContainerItemProxy',containerPortal=raw(project_id),proxyType='1',remoteGlobalIDString=raw(target_ids[dependency_target]),remoteInfo=dependency_target)
            dependency = add('dependency:'+target,'PBXTargetDependency',target=raw(target_ids[dependency_target]),targetProxy=raw(proxy))
            dependencies.append(raw(dependency))
            if not test:
                embed = add('embed:file','PBXBuildFile',fileRef=raw(product_refs['HomeHubShare']),settings={'ATTRIBUTES':['RemoveHeadersOnCopy']})
                phases.append(raw(add('embed:phase','PBXCopyFilesBuildPhase',buildActionMask='2147483647',dstPath='',dstSubfolderSpec='13',files=[raw(embed)],name='Embed App Extensions',runOnlyForDeploymentPostprocessing='0')))
        base = {'SWIFT_VERSION':'5.0','PRODUCT_NAME':'$(TARGET_NAME)','PRODUCT_MODULE_NAME':target,
            'PRODUCT_BUNDLE_IDENTIFIER': 'com.homehub.mac' if platform == 'macos' and not test else 'com.homehub.ios.share' if share else 'com.homehub.ios' if not test else 'com.homehub.'+target.lower(),
            'CODE_SIGN_STYLE':'Automatic','DEVELOPMENT_TEAM':'','GENERATE_INFOPLIST_FILE':'NO',
            'SDKROOT':'macosx' if platform == 'macos' else 'iphoneos',
            'MACOSX_DEPLOYMENT_TARGET':'14.0','IPHONEOS_DEPLOYMENT_TARGET':'17.0','CLANG_ENABLE_MODULES':'YES',
            'ENABLE_HARDENED_RUNTIME':'YES','SWIFT_STRICT_CONCURRENCY':'minimal'}
        if platform == 'ios': base.update(TARGETED_DEVICE_FAMILY='1,2',SUPPORTS_MACCATALYST='NO')
        if test:
            base.update(GENERATE_INFOPLIST_FILE='YES',TEST_HOST='$(BUILT_PRODUCTS_DIR)/'+main+'.app/'+('Contents/MacOS/' if platform == 'macos' else '')+main,BUNDLE_LOADER='$(TEST_HOST)')
        else:
            base.update(INFOPLIST_FILE=target+'/Info.plist',CODE_SIGN_ENTITLEMENTS=target+'/'+target+'.entitlements')
            if share: base.update(APPLICATION_EXTENSION_API_ONLY='YES',SKIP_INSTALL='YES')
        configs=[]
        for config in ['Debug','Release']:
            settings=dict(base,SWIFT_OPTIMIZATION_LEVEL='-Onone' if config == 'Debug' else '-O',ENABLE_TESTABILITY='YES' if config == 'Debug' else 'NO')
            configs.append(raw(add('config:'+target+config,'XCBuildConfiguration',name=config,buildSettings=settings)))
        config_list=add('configs:'+target,'XCConfigurationList',buildConfigurations=configs,defaultConfigurationIsVisible='0',defaultConfigurationName='Release')
        add('target:'+target,'PBXNativeTarget',name=target,productName=target,buildConfigurationList=raw(config_list),buildPhases=phases,buildRules=[],dependencies=dependencies,productReference=raw(product_refs[target]),productType='com.apple.product-type.bundle.unit-test' if test else 'com.apple.product-type.app-extension' if share else 'com.apple.product-type.application')
    products=add('products','PBXGroup',name='Products',sourceTree='<group>',children=[raw(p) for p in product_refs.values()])
    group=add('group','PBXGroup',sourceTree='<group>',children=[raw(v) for v in refs.values()]+[raw(v) for v in localization.values()]+[raw(products)])
    configs=[raw(add('project-config:'+c,'XCBuildConfiguration',name=c,buildSettings={'ALWAYS_SEARCH_USER_PATHS':'NO','CLANG_ENABLE_MODULES':'YES','DEBUG_INFORMATION_FORMAT':'dwarf' if c=='Debug' else 'dwarf-with-dsym'})) for c in ['Debug','Release']]
    config_list=add('project-configs','XCConfigurationList',buildConfigurations=configs,defaultConfigurationIsVisible='0',defaultConfigurationName='Release')
    add('project','PBXProject',attributes={'BuildIndependentTargetsInParallel':'YES','LastUpgradeCheck':'1600'},buildConfigurationList=raw(config_list),compatibilityVersion='Xcode 14.0',developmentRegion='en',knownRegions=['en','hi','Base'],mainGroup=raw(group),productRefGroup=raw(products),projectDirPath='',projectRoot='',targets=[raw(target_ids[t]) for t in targets])
    path=folder/(main+'.xcodeproj'); path.mkdir(exist_ok=True)
    body='// !$*UTF8*$!\n{ archiveVersion = 1; classes = {}; objectVersion = 56; objects = {\n'
    body+='\n'.join(f'{uid} = {serialize(obj)};' for uid,obj in objects.items())
    body+='\n}; rootObject = '+project_id+'; }\n'
    (path/'project.pbxproj').write_text(body)
    scheme_dir=path/'xcshareddata/xcschemes'; scheme_dir.mkdir(parents=True,exist_ok=True)
    def reference(t): return f'<BuildableReference BuildableIdentifier="primary" BlueprintIdentifier="{target_ids[t]}" BuildableName="{t}.app" BlueprintName="{t}" ReferencedContainer="container:{main}.xcodeproj"/>'
    scheme=f'''<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion="1600" version="1.3"><BuildAction parallelizeBuildables="YES" buildImplicitDependencies="YES"><BuildActionEntries><BuildActionEntry buildForTesting="YES" buildForRunning="YES" buildForProfiling="YES" buildForArchiving="YES" buildForAnalyzing="YES">{reference(main)}</BuildActionEntry></BuildActionEntries></BuildAction><TestAction buildConfiguration="Debug"><Testables><TestableReference skipped="NO">{reference(main+'Tests').replace(main+'Tests.app',main+'Tests.xctest')}</TestableReference></Testables></TestAction><LaunchAction buildConfiguration="Debug" selectedDebuggerIdentifier="Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier="Xcode.IDEFoundation.Launcher.LLDB" launchStyle="0" useCustomWorkingDirectory="NO" ignoresPersistentStateOnLaunch="NO" debugDocumentVersioning="YES" allowLocationSimulation="YES"><BuildableProductRunnable runnableDebuggingMode="0">{reference(main)}</BuildableProductRunnable></LaunchAction><ProfileAction buildConfiguration="Release"><BuildableProductRunnable runnableDebuggingMode="0">{reference(main)}</BuildableProductRunnable></ProfileAction><AnalyzeAction buildConfiguration="Debug"/><ArchiveAction buildConfiguration="Release" revealArchiveInOrganizer="YES"/></Scheme>'''
    (scheme_dir/(main+'.xcscheme')).write_text(scheme)

if __name__ == '__main__':
    project('macos'); project('ios')
