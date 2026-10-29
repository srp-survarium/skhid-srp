package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 import flash.geom.Point;
 public class WeaponDetailsHarness extends Sprite {
  private var loader:Loader=new Loader(),domain:ApplicationDomain;
  private var Scene:Object,Details:Object,Loc:Object;
  private var DetailsView:Class,Caption:Class;
  private var cards:Array=[],items:Array=[];
  private var frame:int=0,compactHeight:Number;
  public function WeaponDetailsHarness() {
   stage.align=StageAlign.TOP_LEFT;stage.scaleMode=StageScaleMode.NO_SCALE;
   graphics.beginFill(0x202823);graphics.drawRect(0,0,960,720);graphics.endFill();
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void{e.preventDefault();if(frame>0)finish("uncaught at frame "+frame+": "+e.error);});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function require(ok:Boolean,message:String):void {if(!ok)throw new Error(message);}
  // What the exe gives the movie in `language`: srp's texts (HarnessText, from the dictionaries) and the stock keys the card reads.
  private function texts(language:String):Object {var t:Object={btn_friends:"Friends",clan_players_level_label:"Level",critical_damages:"Critical damage"};for(var k:String in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];return t;}
  private function item(pkp:Boolean):Object {
   var Data:Class=domain.getDefinition("survarium.data.ItemData") as Class;
   var d:Object=new Data();d.dictId=pkp?2229:3;d.name=pkp?"PKP Pecheneg":"AK-74M";d.isWeapon=true;d.category=16;d.superCategory=2;d.itemLevel=15;d.modifiers=[];d.modifiersDefault=[];d.ammoText="7.62x54";d.properties=[];
   var main:Array=pkp?[45,.1,45,650,7,50,50,.4,100]:[34,.2,35,600,3.4,70,40,.25,30];
   var ids:Array=[1,2,3,4,15,24,132,133,134];
   for(var i:int=0;i<ids.length;i++)d.properties.push({prop_id:ids[i],prop_value:String(main[i])});
   var extras:Array=pkp?[10,30,50,.6,1,1,1.4,20,2.5,3,2,2.5,2,2.5,2,3,10,20,.05,.025,5,20]:[3,30,70,.6,.6,.6,1.4,20,1.8,2,1.4,1.8,1.4,1.8,null,2,10.0000001,20,.05,.025,5,20];
   for(i=0;i<extras.length;i++)if(extras[i]!==null)d.properties.push({prop_id:138+i,prop_value:String(extras[i])});
   return d;
  }
  private function recoilItem(topLow:Number,topHigh:Number,leftLow:Number,leftHigh:Number,rightLow:Number,rightHigh:Number):Object {
   var d:Object=item(false),bounds:Array=[topLow,topHigh,leftLow,leftHigh,rightLow,rightHigh];
   for each(var p:Object in d.properties)if(p.prop_id>=146 && p.prop_id<=151)p.prop_value=String(bounds[p.prop_id-146]);
   return d;
  }
  private function stat(d:Object,id:int):Object {
   for each(var row:Object in Details.read(d).rows(true))if(row.kind=="stat" && row.id==id)return row;
   return null;
  }
  private function recoilChecks():void {
   var symmetric:Object=recoilItem(1.8,2,1.4,1.8,1.4,1.8);
   require(stat(symmetric,146).value=="1.8…2" && stat(symmetric,147).value=="0.058","upward range and uniform standard deviation");
   require(stat(symmetric,148).value=="-0.4…0.4" && stat(symmetric,149).value=="0.163","sideways difference range and independent variance");
   var fixed:Object=recoilItem(2,2,3,3,3,3);
   require(stat(fixed,148).value=="0…0" && stat(fixed,149).value=="0","fixed symmetric inputs cancel, including variability");
   var biased:Object=recoilItem(2,2,2,2,3,3);
   require(stat(biased,148).value=="1…1" && stat(biased,149).value=="0","constant sideways bias has zero variability");
   symmetric.properties=symmetric.properties.filter(function(p:Object,index:int,all:Array):Boolean{return p.prop_id!=149;});
   require(stat(symmetric,148)==null && stat(symmetric,149)==null && stat(symmetric,146)!=null,"missing side bound omits side stats only");
   var crossing:Array=[recoilItem(1,4,2,2,3,3),recoilItem(2,3,3,3,3,3)];
   for(var i:int=0;i<2;i++){cards[i].data=crossing[i];cards[i].itemCompare.setSecondaryWeapon(crossing[1-i]);}
   require(color(cards[0],"Vertical recoil")==15911311 && color(cards[1],"Vertical recoil")==15911311,"crossing upward ranges stay neutral");
   require(color(cards[0],"Horizontal recoil")==15911311 && color(cards[1],"Horizontal recoil")==15911311,"signed sideways ranges stay neutral");
   require(color(cards[0],"Horizontal recoil variation")==15911311 && color(cards[1],"Horizontal recoil variation")==15911311,"equal variability stays neutral despite different bias");
   for(i=0;i<2;i++){cards[i].data=items[i];cards[i].itemCompare.setSecondaryWeapon(items[1-i]);}
  }
  private function loaded(e:Event):void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;Scene=domain.getDefinition("MainScene");Scene.stage=this;Scene.scaleFactor=1;
    var css:StyleSheet=new StyleSheet();css.setStyle("red",{color:"#CF4242"});css.setStyle("white",{color:"#D7DEDC"});css.setStyle("gray",{color:"#8A9490"});Scene.textStyleSheet=css;
    Loc=domain.getDefinition("survarium.data.LocalizationData");Loc.setData(texts("english"));
    Details=domain.getDefinition("survarium.item.WeaponDetails");
    DetailsView=domain.getDefinition("survarium.item.WeaponDetailsView") as Class;
    Caption=domain.getDefinition("survarium.item.WeaponDetailCaption") as Class;
    var Props:Class=domain.getDefinition("survarium.data.PropertiesDescriptor") as Class;
    var props:Object=new Props("properties"),list:Array=[];
    for each(var spec:Array in [[1,"Damage",1,""],[2,"Dispersion",0,""],[3,"Armor penetration",1,""],[4,"Rate of fire",1,""],[15,"Weight",0,"kg"],[24,"Distance",1,"m"],[132,"Stopping power",1,"%"],[133,"Aim time",0,"s"],[134,"Magazine",1,""]])list.push({prop_id:spec[0],prop_name:spec[1],direction:spec[2],postfix:spec[3],comparable:true,min_value:0,max_value:100});
    for(var id:int=138;id<=159;id++)list.push({prop_id:id,comparable:false});
    props.setPropertiesDictionary(list);domain.getDefinition("survarium.controls.ItemCompare").setPropsDescriptor(props);
    var Container:Class=domain.getDefinition("survarium.data.IconContainer") as Class;
    var Atlas:Class=domain.getDefinition("LobbyMenu_ModifierIcons") as Class;
    new Container("modifiers",38,38,Bitmap(new Atlas()).bitmapData,true,0,0,0,false);
    items=[item(true),item(false)];
    var Descriptor:Class=domain.getDefinition("survarium.data.ItemDescriptor") as Class;
    var descriptor:Object=new Descriptor("InventoryItems");descriptor.setItemsDictionary([{dictId:3,item_properties:items[1].properties}]);
    var Hint:Class=domain.getDefinition("InventoryItemHint") as Class;
    for(var i:int=0;i<2;i++){var card:Object=new Hint();card.x=40+i*440;card.y=20;card.scaleX=card.scaleY=.75;addChild(card as DisplayObject);cards.push(card);}
    addEventListener(Event.ENTER_FRAME,tick);
   }catch(err:Error){finish("setup "+err+" "+err.getStackTrace());}
  }
  private function text(card:Object):String {
   var out:String="";
   for(var i:int=0;i<card.numChildren;i++){
    var child:Object=card.getChildAt(i);
    if(child is DetailsView && child.visible){
     for(var j:int=0;j<child.numChildren;j++){
      var element:Object=child.getChildAt(j);
      if(element is Caption){
       out+=element.textField.text+"\n";
       require(element.textField.textWidth+4<=element.textField.width/element.textField.scaleX+.1,"caption fits card: "+element.textField.text);
       if(element.kind=="heading")require(element.textField.getTextFormat().size>card.itemCompare.diff_1.indicator.textFieldLabel.getTextFormat().size && element.textField.textColor==0xD7DEDC,"headings visibly distinct");
       if(element.kind=="hint")require(element.textField.textColor==0x7B8581,"Shift hint muted gray");
       continue;
      }
      var row:Object=child.getChildAt(j).indicator;
      var original:Object=card.itemCompare.diff_1.indicator;
      out+=row.textFieldLabel.text+": "+row.textFieldValue.text+"\n";
      for each(var name:String in ["textFieldLabel","textFieldValue"]){
       var actual:TextFormat=row[name].getTextFormat(),expected:TextFormat=original[name].getTextFormat();
       require(!row[name].text.length || (actual.font==expected.font && actual.size==expected.size && actual.bold==expected.bold && actual.align==expected.align),"original typography: "+name+" '"+row[name].text+"' got="+[actual.font,actual.size,actual.bold,actual.align]+" expected="+[expected.font,expected.size,expected.bold,expected.align]);
       require(Math.abs(row[name].localToGlobal(new Point()).x-original[name].localToGlobal(new Point()).x)<.2,"original column alignment: "+name);
       require(Math.abs(row[name].transform.concatenatedMatrix.a-original[name].transform.concatenatedMatrix.a)<.001 && Math.abs(row[name].transform.concatenatedMatrix.d-original[name].transform.concatenatedMatrix.d)<.001,"original text scale: "+name);
      }
      require(row.textFieldLabel.textColor==original.textFieldLabel.textColor,"original label color");
      require([15911311,8890390,9834254].indexOf(row.textFieldValue.textColor)>=0,"original stat palette");
      require(row.textFieldValue.textWidth<row.textFieldValue.width,"value fits row: "+row.textFieldValue.text);
      if(j+1<child.numChildren && !(child.getChildAt(j+1) is Caption) && row.textFieldLabel.numLines==1)require(Math.abs(child.getChildAt(j+1).y-child.getChildAt(j).y-(card.itemCompare.diff_2.y-card.itemCompare.diff_1.y))<.1,"original row spacing");
     }
    }
   }
   return out;
  }
  // Every translated stat label is one line and ends left of its value's text, with room to spare:
  // Ruffle draws the embedded Calibri, yet not exactly as Scaleform lays it out, so an exact fit proves nothing.
  // The captions (headings, the Shift hint) are one line too. Every miss of the card, measured.
  private static const FIT_MARGIN:Number=0.05;
  private function overflows(card:Object,language:String):Array {
   var view:Object=detailsView(card),misses:Array=[];
   for(var i:int=0;i<view.numChildren;i++){
    var e:Object=view.getChildAt(i);
    if(e is Caption){
     var field:TextField=e.textField;
     if(field.numLines!=1 || field.textWidth*(1+FIT_MARGIN)+4>field.width)misses.push(language+": caption '"+field.text+"' "+Math.round(field.textWidth)+"/"+Math.round(field.width-4));
     continue;
    }
    var label:TextField=e.indicator.textFieldLabel,value:TextField=e.indicator.textFieldValue;
    var align:String=value.getTextFormat().align,valueStart:Number=value.x;
    if(align==TextFormatAlign.RIGHT)valueStart+=value.width-value.textWidth-4;
    else if(align==TextFormatAlign.CENTER)valueStart+=(value.width-value.textWidth)/2-2;
    if(label.numLines!=1 || label.x+label.textWidth*(1+FIT_MARGIN)+6>valueStart)misses.push(language+": '"+label.text+"' "+Math.round(label.textWidth)+"/"+Math.round(valueStart-label.x-6)+(label.numLines!=1?" ("+label.numLines+" lines)":""));
   }
   return misses;
  }
  private function detailsView(card:Object):Object {
   for(var i:int=0;i<card.numChildren;i++)if(card.getChildAt(i) is DetailsView)return card.getChildAt(i);
   throw new Error("missing detail view");
  }
  private function color(card:Object,label:String):uint {
   var view:Object=detailsView(card);
   for(var i:int=0;i<view.numChildren;i++){
    var e:Object=view.getChildAt(i);
    if(!(e is Caption) && e.indicator.textFieldLabel.text==label)return e.indicator.textFieldValue.textColor;
   }
   throw new Error("missing stat: "+label);
  }
  private function neutral(card:Object):void {
   var view:Object=detailsView(card);
   for(var i:int=0;i<view.numChildren;i++){
    var e:Object=view.getChildAt(i);
    if(!(e is Caption))require(e.indicator.textFieldValue.textColor==15911311,"Ctrl suppresses added stat colors");
   }
   for(i=1;i<=9;i++){
    var row:Object=card.itemCompare.getChildByName("diff_"+i);
    if(row && row.visible){
     require(row.indicator.textFieldValue.textColor==15911311,"Ctrl suppresses original stat colors");
     require(!row.indicator.mc_diff_positive.visible && !row.indicator.mc_diff_negative.visible,"Ctrl suppresses comparison bars");
    }
   }
  }
  private function aligned():void {
   var a:Object=detailsView(cards[0]),b:Object=detailsView(cards[1]);
   require(a.numChildren==b.numChildren,"both weapons show the same detail rows");
   for(var i:int=0;i<a.numChildren;i++)require(Math.abs(a.getChildAt(i).localToGlobal(new Point()).y-b.getChildAt(i).localToGlobal(new Point()).y)<.2,"compared row alignment: "+i);
  }
  private function tick(e:Event):void {
   frame++;
   try {
    if(frame==20){cards[0].data=items[0];cards[1].data=items[1];cards[0].itemCompare.setSecondaryWeapon(items[1]);cards[1].itemCompare.setSecondaryWeapon(items[0]);}
    if(frame==35){
     // Native Scaleform can resize the comparison before ButtonBar.configUI
     // has created its container, even with a data provider already assigned.
     for each(var listName:String in ["modifiersList","defaultModifiersList","criticalModifiersList"]){
      var savedContainer:Object=cards[0][listName].container;
      cards[0][listName].container=null;
      try{cards[0].itemCompare.dispatchEvent(new Event(Event.RESIZE));}
      finally{cards[0][listName].container=savedContainer;}
     }
     cards[0].itemCompare.dispatchEvent(new Event(Event.RESIZE));
     require(text(cards[0]).indexOf("Reload: 10s")>=0,"base reload");
     require(text(cards[0]).indexOf("Full damage up to: 30m")>=0 && text(cards[0]).indexOf("Minimum damage from: 50m")>=0 && text(cards[0]).indexOf("Minimum damage: 60%")>=0,"damage falloff endpoints and floor");
     require(text(cards[0]).indexOf("Hold Shift")>=0,"discoverable expanded view");
     compactHeight=cards[0].height;Scene.isShift=true;
    }
    if(frame==45){
     var s:String=text(cards[0]);require(s.indexOf("Vertical recoil: 2.5…3")>=0,"weapon own upward kick range");
     require(s.indexOf("Horizontal recoil: -0.5…0.5")>=0,"weapon own sideways kick range");
     require(s.indexOf("Vertical recoil variation: 0.144")>=0 && s.indexOf("Horizontal recoil variation: 0.204")>=0,"weapon own recoil standard deviations");
     require(s.indexOf("Hip-fire spread: 2x")>=0 && s.indexOf("Draw time: 1s")>=0 && s.indexOf("Holster time: 1s")>=0,"handling and accuracy");
     require(text(cards[1]).indexOf("Hip-fire spread: 1x")>=0,"missing multiplier shows native 1x default, as on Kiparis");
     aligned();
     require(s.indexOf("reliability")<0 && s.indexOf("Wear")<0,"excluded stats stay excluded");
     require(cards[0].height>compactHeight && cards[0].height<900,"expanded native frame fits screen");
     for(var j:int=0;j<cards[0].numChildren;j++){var f:Object=cards[0].getChildAt(j);if(f is DetailsView && f.visible)require(f.y+f.height<cards[0].background.height,"details enclosed by native frame");}
     require(s.indexOf("AK-74M")<0,"no invented reference parameter");
     require(s.indexOf("Weapon modifiers only")<0,"no explanatory note presented as a stat");
     require(color(cards[0],"Reload")==9834254 && color(cards[1],"Reload")==8890390,"lower reload is green, higher reload is red");
     require(color(cards[0],"Minimum damage from")==9834254 && color(cards[1],"Minimum damage from")==8890390,"higher falloff distance is green");
     require(color(cards[0],"Vertical recoil")==9834254 && color(cards[1],"Vertical recoil")==8890390,"upward range comparison colors");
     require(color(cards[0],"Vertical recoil variation")==9834254 && color(cards[1],"Vertical recoil variation")==8890390,"upward variability comparison colors");
     require(color(cards[0],"Horizontal recoil variation")==9834254 && color(cards[1],"Horizontal recoil variation")==8890390,"sideways variability comparison colors");
     require(color(cards[0],"Horizontal recoil")==15911311 && color(cards[1],"Horizontal recoil")==15911311,"signed sideways range is neutral");
     require(color(cards[0],"Minimum damage")==15911311 && color(cards[0],"Spread recovery rate")==15911311,"equal values retain their normal color");
     Scene.isCtrl=true;
    }
    if(frame==55){neutral(cards[0]);neutral(cards[1]);Scene.isCtrl=false;}
    if(frame==65){require(color(cards[0],"Reload")==9834254,"release Ctrl restores comparisons");require(cards[0].itemCompare.diff_1.indicator.textFieldValue.textColor==8890390,"release Ctrl restores original comparison colors");Scene.isShift=false;}
    if(frame==75){require(Math.abs(cards[0].height-compactHeight)<1,"release Shift restores compact layout");Scene.isCtrl=true;}
    if(frame==85){neutral(cards[0]);neutral(cards[1]);require(text(cards[0]).indexOf("Hold Shift")>=0,"Ctrl alone retains compact layout");Scene.isCtrl=false;}
    if(frame==95){require(color(cards[0],"Reload")==9834254,"release Ctrl alone restores comparisons");Scene.isCtrl=true;}
    if(frame==100){neutral(cards[0]);Scene.isShift=true;}
    if(frame==110){neutral(cards[0]);neutral(cards[1]);require(text(cards[0]).indexOf("Vertical recoil")>=0,"Shift expands while Ctrl remains neutral");Scene.isCtrl=false;}
    if(frame==120)require(color(cards[0],"Reload")==9834254,"Ctrl-first chord restores comparisons");

    if(frame==130){
     // The widest values of the live weapons (MSBS: sideways -0.58…0.58, upward 1.98…2.2) leave a label the least room.
     var misses:Array=[],wide:Object=recoilItem(1.98,2.2,1.42,2,1.42,2);
     for each(var language:String in HarnessText.LANGUAGES){
      Loc.setData(texts(language));
      Scene.isShift=false;cards[0].data=items[0];cards[1].data=items[1];
      require(text(cards[0]).indexOf(HarnessText.DATA[language].st_srp_weapon_shift_hint)>=0,"compact card: "+language);
      misses=misses.concat(overflows(cards[0],language));
      Scene.isShift=true;cards[0].data=items[0];cards[1].data=wide;
      misses=misses.concat(overflows(cards[0],language),overflows(cards[1],language));
      cards[1].data=items[1];
      require(cards[0].height<900,"expanded translated card fits: "+language);
      var translated:String=text(cards[0]);
      require(translated.indexOf("undefined")<0 && translated.indexOf("#st_")<0 && translated.indexOf("%1$s")<0,"complete translation: "+language);
      require(translated.indexOf(HarnessText.DATA[language].st_srp_weapon_kick_up+": 2.5…3")>=0,"translated label and value: "+language);
      require(translated.indexOf(HarnessText.DATA[language].st_srp_weapon_reload+": "+HarnessText.DATA[language].st_srp_unit_seconds.split("%1$s").join("10"))>=0,"translated unit: "+language);
      aligned();
     }
     var told:Object={};misses=misses.filter(function(m:String,...rest):Boolean{var again:Boolean=told[m];told[m]=true;return !again;});
     for each(var miss:String in misses)trace("DETAILS OVERFLOW "+miss);
     require(misses.length==0,misses.length+" labels do not fit one line (DETAILS OVERFLOW lines)");
     Loc.setData(texts("english"));cards[0].data=items[0];cards[1].data=items[1];
     recoilChecks();
     var baseRange:String=text(cards[0]);
     var defaultRange:Object={props:[{prop_id:24,prop_value:"0.10"}],isDefault:true,icon:33,textValue:"Range: +10%"};
     items[0].modifiersDefault=[defaultRange];
     items[0].modifiers=[defaultRange,{icon:33,textValue:"Range +15%, reload +20%",props:[{prop_id:24,prop_value:"0.15"},{prop_id:114,prop_value:"0.20"}]},{state:1,icon:36,textValue:"Range -10%, reload -10%",props:[{prop_id:24,prop_value:"-0.10"},{prop_id:114,prop_value:"-0.10"}]}];
     cards[0].data=items[0];cards[1].data=items[1];
     var modified:String=text(cards[0]);
     require(modified.indexOf("Full damage up to: 34.5m")>=0 && modified.indexOf("Minimum damage from: 57.5m")>=0,"range bonuses and defects affect both distances, defaults once");
     require(modified.indexOf("Reload: 9s")>=0,"weapon reload modifier uses correct sign and summed percentages");
     require(color(cards[0],"Full damage up to")==8890390 && color(cards[1],"Full damage up to")==9834254,"comparison uses modified distance");
     require(cards[0].height<1080,"expanded card with bonuses and a defect fits the game's logical viewport");
     cards[0].data=items[0];require(text(cards[0])==modified,"reopening does not apply modifiers twice");
     var weaponOnly:Object={isWeapon:true,properties:items[0].properties,modifiers:[],gloves:{modifiers:[{props:[{prop_id:114,prop_value:"0.50"}]}]},profileModifiers:[{props:[{prop_id:24,prop_value:"0.50"}]}]};
     var ownRows:Array=Details.read(weaponOnly).rows(true);
     for each(var ownRow:Object in ownRows)if(ownRow.kind=="stat" && ownRow.id==138)require(ownRow.value=="10s","gloves and profile modifiers are not weapon modifiers");
     items[0].modifiers=[];items[0].modifiersDefault=[];cards[0].data=items[0];cards[1].data=items[1];
     require(text(cards[0])==baseRange,"base descriptor preserved after modifiers");
     // Missing metadata must not remove the original distance statistic.
     require(Details.read({isWeapon:true,properties:[{prop_id:24,prop_value:"50"}]})==null,"old content fallback");
     require(Details.read({isWeapon:false,properties:items[0].properties})==null,"nonweapon cards unchanged");
     var headerHeight:Number=cards[0].topBlackContainer.height;
     cards[1].textFieldDescription.y=52;
     cards[1].textFieldDescription.autoSize=TextFieldAutoSize.LEFT;
     cards[1].textFieldDescription.text="Optics / collimators: Stock scope\nLong attached optics name";
     cards[1].textFieldDescription.visible=true;
     cards[0].alignComparisonHeader(cards[1]);
     require(cards[0].topBlackContainer.height>headerHeight,"reserve wrapped optics header on both cards");
     require(cards[0].itemCompare.y==cards[1].itemCompare.y,"original stats aligned with optics on one weapon");
     aligned();
     cards[1].alignComparisonHeader(cards[0]);aligned();
     cards[0].alignComparisonHeader(null);
     require(Math.abs(cards[0].topBlackContainer.height-headerHeight)<.1,"single card restores its natural header");
     cards[1].textFieldDescription.visible=false;cards[0].alignComparisonHeader(cards[1]);aligned();
     finish(null);
    }
   }catch(err:Error){finish("frame "+frame+": "+err+" "+err.getStackTrace());}
  }
  private function finish(error:String):void {removeEventListener(Event.ENTER_FRAME,tick);if(!error){Scene.isCtrl=loaderInfo.parameters.neutral=="1";if(loaderInfo.parameters.compact=="1")Scene.isShift=false;}trace("DETAILS RESULT "+(error?"fail: "+error:"pass"));}
 }
}
