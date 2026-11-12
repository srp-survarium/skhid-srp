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
  private var labels:Array = ["Reduce detail with distance (off = maximum)",
   "Details mit Entfernung reduzieren (aus = maximal)",
   "Zmniejszaj szczegóły z odległością (wył. = maksimum)",
   "Снижать детализацию вдали (выкл. = максимум)",
   "Снижать детализацию вдали (выкл. = максимум)",
   "Знижувати деталізацію вдалині (вимк. = максимум)"];
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
   for(var i:int=0;i<20;i++) options.push({label:i==19?"Reduce detail with distance (off = maximum)":"Option " + i,setter:function(value:*):* { setterCalls++; return value; }});
   settings.setSettings(2,options);
   settings.setSettings(0,[]);
   settings.selectIndex(2);
  }
  private function check():void {
   var content:Object=settings.settings_content;
   var box:Object=content["_distance_detail"];
   if(!box || !box.visible) { failures.push("missing graphics checkbox"); return; }
   if(box.y < content._motion_blur.y + content._motion_blur.height) failures.push("overlaps motion blur");
   var count:int=0;
   for(var i:int=0;i<content.numChildren;i++) if(content.getChildAt(i).name=="_distance_detail") count++;
   if(count!=1) failures.push("checkbox duplicated: " + count);
   if(box.textField.textWidth > box.textField.width - 3 || box.textField.textHeight > box.textField.height - 3) failures.push("label clipped");
   trace("DETAIL text width="+box.textField.textWidth+"/"+box.textField.width+" height="+box.textField.textHeight+"/"+box.textField.height+" wrap="+box.textField.wordWrap);
   trace("DETAIL checkbox x="+box.x+" y="+box.y+" width="+box.width+" height="+box.height+" text="+box.textField.text+" bottom="+box.getBounds(DisplayObject(settings)).bottom+" buttons="+settings.btn_accept.y);
   if(box.getBounds(DisplayObject(settings)).bottom > settings.btn_accept.y) failures.push("overlaps buttons");
  }
  private function tick(e:Event):void {
   frame++;
   try {
    if(frame==10) setup();
    if(frame==20) { check(); settings.setValue(2,19,1); }
    if(frame==25) {
     var box:Object=settings.settings_content["_distance_detail"];
     if(!box.selected) failures.push("native value not reflected");
     setterCalls=0;
     box.selected=false;
     trace("DETAIL setter calls="+setterCalls+" apply="+settings.btn_accept.enabled);
     if(setterCalls<1 || !settings.btn_accept.enabled) failures.push("checkbox does not bind native setter");
     settings.btn_accept.enabled=false; settings.selectIndex(0);
    }
    if(frame==30) { if(settings.settings_content["_distance_detail"].visible) failures.push("visible on game tab"); settings.selectIndex(2); }
    if(frame==40) check();
    if(frame>=45 && frame<105 && frame%10==5) settings.settings_content["_distance_detail"].label=labels[(frame-45)/10];
    if(frame>=50 && frame<=100 && frame%10==0) check();
    if(frame==105) finish(null);
   } catch(err:Error) { finish("frame " + frame + ": " + err + " " + err.getStackTrace()); }
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick); if(error) failures.push(error);
   trace("DETAIL RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
   if(loaderInfo.parameters.hold != "1") fscommand("quit");
  }
 }
}
