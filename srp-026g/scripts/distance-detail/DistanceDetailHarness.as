package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 public class DistanceDetailHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var settings:Object;
  private var failures:Array = [];
  private var frame:int = 0;
  private var setterCalls:int = 0;
  private var lastNative:int = -1;
  private var lastPreset:int = -1;
  private var labels:Array = ["Distance detail|Normal|Maximum", "Detailgrad|Normal|Maximal", "Szczegółowość|Normalna|Maksymalna", "Детализация|Обычная|Максимум", "Детализация|Обычная|Максимум", "Деталізація|Звичайна|Максимум"];
  public function DistanceDetailHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.load(new URLRequest("main_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function loaded(e:Event):void {
   try {
    var Window:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("SrvSettingsWndow") as Class;
    settings = new Window(); addChild(DisplayObject(settings));
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) { finish("setup " + err); }
  }
  private function setup():void {
   settings.setLabels([{name:"gameplay_options_type",label:"Game"},{name:"controllers_options_type",label:"Controls"},{name:"video_options_type",label:"Graphics"},{name:"sound_options_type",label:"Sound"}]);
   var options:Array=[];
   for(var i:int=0;i<20;i++) options.push({label:i==19?labels[0]:"Option " + i,setter:function(value:*):* { setterCalls++; lastNative=int(value); return value; }});
   options[8].dataProvider=["Minimum","Low","Medium","High","Ultra","Custom"];
   options[8].setter=function(value:*):* { lastPreset=int(value); return value; };
   settings.setSettings(2,options);
   settings.setSettings(0,[]);
   settings.selectIndex(2);
  }
  private function check():void {
   var content:Object=settings.settings_content;
   var box:Object=content["_distance_detail"];
   if(!box || !box.visible) { failures.push("missing graphics picker"); return; }
   if(box.y < content._motion_blur.y + content._motion_blur.height) failures.push("overlaps motion blur");
   var count:int=0;
   for(var i:int=0;i<content.numChildren;i++) if(content.getChildAt(i).name=="_distance_detail") count++;
   if(count!=1) failures.push("picker duplicated: " + count);
   if(box.textField.textWidth > box.textField.width - 3 || box.textField2.textWidth > box.textField2.width - 3) failures.push("label clipped");
   trace("DETAIL choice="+box.textField2.text+" width="+box.textField2.textWidth+"/"+box.textField2.width+" x="+box.textField2.x+" next="+box.nextBtn.x+" prev="+box.prevBtn.x);
   trace("DETAIL text width="+box.textField.textWidth+"/"+box.textField.width+" height="+box.textField.textHeight+"/"+box.textField.height+" wrap="+box.textField.wordWrap);
   trace("DETAIL picker x="+box.x+" y="+box.y+" width="+box.width+" height="+box.height+" text="+box.textField.text+" bottom="+box.getBounds(DisplayObject(settings)).bottom+" buttons="+settings.btn_accept.y);
   if(box.getBounds(DisplayObject(settings)).bottom > settings.btn_accept.y) failures.push("overlaps buttons");
  }
  private function tick(e:Event):void {
   frame++;
   try {
    if(frame==10) setup();
    if(frame==20) { check(); settings.setValue(2,19,1); }
    if(frame==25) {
     var box:Object=settings.settings_content["_distance_detail"];
     if(box.selectedIndex != 0) failures.push("native adaptive value not reflected");
     setterCalls=0;
     box.selectedIndex=1; box.validateNow();
     trace("DETAIL setter calls="+setterCalls+" apply="+settings.btn_accept.enabled);
     if(setterCalls<1 || lastNative != 0 || !settings.btn_accept.enabled) failures.push("picker does not bind native setter");
     settings.btn_accept.enabled=false; settings.selectIndex(0);
    }
    if(frame==30) { if(settings.settings_content["_distance_detail"].visible) failures.push("visible on game tab"); settings.selectIndex(2); }
    if(frame==40) { check(); settings.setValues(2,[{id:19,value:0}]); }
    if(frame==43) {
     if(settings.settings_content["_distance_detail"].selectedIndex != 1) failures.push("bulk native maximum value not reflected");
     settings.setValue(2,19,1);
    }
    if(frame==44 && settings.settings_content["_distance_detail"].selectedIndex != 0) failures.push("native reset not reflected");
    if(frame>=45 && frame<105 && frame%10==5) {
     var parts:Array=labels[(frame-45)/10].split("|");
     var dp:Class=loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.data.DataProvider") as Class;
     settings.settings_content["_distance_detail"].label=parts[0];
     settings.settings_content["_distance_detail"].dataProvider=new dp([parts[1],parts[2]]);
     settings.settings_content["_distance_detail"].selectedIndex=1;
    }
    if(frame>=50 && frame<=100 && frame%10==0) check();
    if(frame==105) {
     settings.setValues(2,[{id:8,value:5},{id:19,value:1}]);
    }
    if(frame>=110 && frame<=150 && frame%10==0) {
     var presetIndex:int=(frame-110)/10;
     var preset:Object=settings.settings_content["_overall_graphics"];
     preset.selectedIndex=presetIndex; preset.validateNow();
    }
    if(frame>=115 && frame<=155 && frame%10==5) {
     var expectedPreset:int=(frame-115)/10;
     var expectedDetail:int=expectedPreset==4?1:0;
     if(settings.settings_content["_overall_graphics"].selectedIndex!=expectedPreset || lastPreset!=expectedPreset) failures.push("preset changed to Custom during selection "+expectedPreset);
     if(settings.settings_content["_distance_detail"].selectedIndex!=expectedDetail) failures.push("preset does not set detail "+expectedPreset);
     if(expectedPreset==4 && lastNative!=0) failures.push("Ultra did not stage native maximum");
     trace("DETAIL preset="+expectedPreset+" detail="+settings.settings_content["_distance_detail"].selectedIndex);
    }
    if(frame==160) {
     settings.settings_content["_distance_detail"].selectedIndex=0;
     settings.settings_content["_distance_detail"].validateNow();
    }
    if(frame==165) {
     if(settings.settings_content["_overall_graphics"].selectedIndex!=5 || lastPreset!=5 || lastNative!=1) failures.push("manual Normal did not stage Custom");
     settings.settings_content["_overall_graphics"].selectedIndex=3;
     settings.settings_content["_overall_graphics"].validateNow();
    }
    if(frame==170) {
     settings.settings_content["_distance_detail"].selectedIndex=1;
     settings.settings_content["_distance_detail"].validateNow();
    }
    if(frame==175) {
     if(settings.settings_content["_overall_graphics"].selectedIndex!=5 || lastPreset!=5 || lastNative!=0) failures.push("manual Maximum did not stage Custom");
     // Cancel/reopen supplies the saved native pair. Loading it must not
     // run either setter or turn a saved preset into Custom.
     setterCalls=0; lastPreset=-1;
     settings.setValues(2,[{id:8,value:4},{id:19,value:0}]);
    }
    if(frame==180) {
     if(settings.settings_content["_overall_graphics"].selectedIndex!=4 || settings.settings_content["_distance_detail"].selectedIndex!=1) failures.push("saved Ultra/Maximum pair not restored");
     settings.selectIndex(0);
    }
    if(frame==185) settings.selectIndex(2);
    if(frame==190) {
     if(settings.settings_content["_overall_graphics"].selectedIndex!=4 || settings.settings_content["_distance_detail"].selectedIndex!=1) failures.push("preset pair lost on tab revisit");
     finish(null);
    }
   } catch(err:Error) { finish("frame " + frame + ": " + err + " " + err.getStackTrace()); }
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick); if(error) failures.push(error);
   trace("DETAIL RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
   if(loaderInfo.parameters.hold != "1") fscommand("quit");
  }
 }
}
