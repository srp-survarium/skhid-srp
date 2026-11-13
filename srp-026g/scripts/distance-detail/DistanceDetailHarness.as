package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 public class DistanceDetailHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var fixtureLoader:URLLoader = new URLLoader();
  private var nativeMenu:Object;
  private var settings:Object;
  private var failures:Array = [];
  private var frame:int = 0;
  private var pendingPreset:int = 3;
  private var pendingDetail:int = 1;
  private var detailCalls:int = 0;
  private var labels:Array;
  public function DistanceDetailHarness() {
   fixtureLoader.addEventListener(Event.COMPLETE,fixtureLoaded);
   fixtureLoader.addEventListener(IOErrorEvent.IO_ERROR,function(e:Event):void { finish("native fixture missing"); });
   fixtureLoader.load(new URLRequest("native-check.json"));
  }
  private function fixtureLoaded(e:Event):void {
   var fixture:Object=JSON.parse(String(fixtureLoader.data));
   nativeMenu=fixture.native_menu;
   labels=fixture.distance_detail_labels;
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.load(new URLRequest("main_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function loaded(e:Event):void {
   try {
    var Window:Class=loader.contentLoaderInfo.applicationDomain.getDefinition("SrvSettingsWndow") as Class;
    settings=new Window(); addChild(DisplayObject(settings));
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) { finish("setup "+err); }
  }
  private function updates(rows:Array):void {
   for each(var row:Array in rows) settings.setValue(row[0],row[1],row[2],Boolean(row[3]));
  }
  private function setter(id:int):Function {
   return function(value:*):uint {
    var index:uint=uint(value);
    if(id==8) {
     pendingPreset=index;
     if(index<5) updates(nativeMenu.presets[index]);
    } else if(id==19) {
     pendingDetail=index; detailCalls++;
     for each(var trace:Object in nativeMenu.manual) {
      if(trace.preset==pendingPreset && trace.choice==index) { updates(trace.updates); break; }
     }
    }
    return index;
   };
  }
  private function setup():void {
   settings.setLabels([{name:"gameplay_options_type",label:"Game"},{name:"controllers_options_type",label:"Controls"},{name:"video_options_type",label:"Graphics"},{name:"sound_options_type",label:"Sound"}]);
   var options:Array=[];
   for(var i:int=0;i<20;i++) {
    var option:Object={label:i==19?"Distance detail":"Option "+i,setter:setter(i)};
    if(i==8) option.dataProvider=["Minimum","Low","Medium","High","Ultra","Custom"];
    if(i>=9 && i<=18) option.dataProvider=["Minimum","Low","Medium","High","Ultra"];
    if(i==19) option.dataProvider=["Normal","High","Maximum"];
    options.push(option);
   }
   settings.setSettings(2,options); settings.setSettings(0,[]); settings.selectIndex(2);
   settings.setValues(2,[{id:8,value:3},{id:19,value:1}]);
  }
  private function get detail():Object { return settings.settings_content["_distance_detail"]; }
  private function get preset():Object { return settings.settings_content["_overall_graphics"]; }
  private function click(button:Object):void {
   if(!button.enabled) { failures.push("clicked arrow/button disabled"); return; }
   var ButtonEvent:Class=loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
   button.dispatchEvent(new ButtonEvent("buttonClick"));
  }
  private function check(expectedPreset:int,expectedDetail:int,pending:Boolean=true):void {
   if(preset.selectedIndex!=expectedPreset || pendingPreset!=expectedPreset) failures.push("preset "+preset.selectedIndex+" native "+pendingPreset+" expected "+expectedPreset);
   if(detail.selectedIndex!=expectedDetail || pendingDetail!=expectedDetail) failures.push("detail "+detail.selectedIndex+" native "+pendingDetail+" expected "+expectedDetail);
   if(pending && !settings.btn_accept.enabled) failures.push("Apply did not enable");
   if(detail.prevBtn.enabled!=(expectedDetail>0) || detail.nextBtn.enabled!=(expectedDetail<2)) failures.push("arrow enable states wrong");
   trace("DETAIL state preset="+preset.selectedIndex+" detail="+detail.selectedIndex+" apply="+settings.btn_accept.enabled+" left="+detail.prevBtn.enabled+" right="+detail.nextBtn.enabled);
  }
  private function tick(e:Event):void {
   frame++;
   try {
    if(frame==10) setup();
    // Real arrow ButtonEvents, not assignments to selectedIndex.
    if(frame==20) click(preset.nextBtn); // High -> Ultra, native callback stages Maximum.
    if(frame==25) { check(4,2); click(preset.prevBtn); }
    if(frame==30) { check(3,1,false); click(detail.prevBtn); }
    if(frame==35) { check(5,0); click(detail.nextBtn); }
    if(frame==40) { check(5,1); click(detail.nextBtn); }
    if(frame==45) { check(5,2); click(detail.prevBtn); }
    if(frame==50) { check(5,1); click(detail.prevBtn); }
    if(frame==52) { check(5,0); click(detail.nextBtn); }
    if(frame==55) {
     check(5,1); click(settings.btn_accept);
     // Native Apply/reinitialize is covered in the fixture producer; replay
     // the saved pair as the player returns it to the menu.
     settings.setValues(2,[{id:8,value:5},{id:19,value:1}]);
    }
    if(frame==60) {
     check(5,1,false);
     if(settings.btn_accept.enabled) failures.push("Apply remained enabled after save");
     click(detail.prevBtn);
    }
    if(frame==65) { check(5,0); click(settings.btn_cancel); settings.setValue(2,19,1,true); }
    if(frame==70) {
     pendingDetail=1; check(5,1,false);
     if(settings.btn_accept.enabled) failures.push("Cancel left pending changes");
     settings.selectIndex(0);
    }
    if(frame==75) { if(detail.visible) failures.push("detail visible outside Graphics"); settings.selectIndex(2); }
    if(frame==80) { check(5,1,false); preset.selectedIndex=0; }
    if(frame>=85 && frame<=125 && frame%10==5) {
     var expected:int=(frame-85)/10;
     check(expected,Math.max(0,expected-2));
     if(expected<4) click(preset.nextBtn);
    }
    if(frame>=130 && frame<130+labels.length*10 && frame%10==0) {
     var texts:Array=labels[(frame-130)/10];
     var DP:Class=loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.data.DataProvider") as Class;
     detail.label=texts[0]; detail.dataProvider=new DP([texts[1],texts[2],texts[3]]);
    }
    if(frame>=135 && frame<135+labels.length*10 && frame%10==5) {
     if(detail.textField.textWidth>detail.textField.width-3 || detail.textField2.textWidth>detail.textField2.width-3) failures.push("translation clipped");
     if(detail.getBounds(DisplayObject(settings)).bottom>settings.btn_accept.y) failures.push("overlaps buttons");
    }
    if(frame==135+labels.length*10) {
     if(detailCalls<8) failures.push("arrows did not reach native setters");
     finish(null);
    }
   } catch(err:Error) { finish("frame "+frame+": "+err+" "+err.getStackTrace()); }
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick); if(error) failures.push(error);
   trace("DETAIL RESULT "+(failures.length?"fail: "+failures.join("; "):"pass"));
   if(loaderInfo.parameters.hold!="1") fscommand("quit");
  }
 }
}
