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
  // A bolt-action AK-74M stand-in: its chambering time (prop 160, the weapons that have one).
  private function boltItem(chamber:String):Object {var d:Object=item(false);d.properties.push({prop_id:160,prop_value:chamber});return d;}
  private function shows(d:Object,other:Object):void {cards[0].data=d;cards[1].data=other;cards[0].itemCompare.setSecondaryWeapon(other);cards[1].itemCompare.setSecondaryWeapon(d);}
  // The native comparison row of a property (diff_1..diff_9), by its label.
  private function nativeRow(card:Object,label:String):Object {
   for(var i:int=1;i<=9;i++){var row:Object=card.itemCompare.getChildByName("diff_"+i);if(row && row.visible && row.indicator.label==label)return row.indicator;}
   throw new Error("missing native row: "+label);
  }
  // weapon-rolls-1: the recoil, aim-time and chambering rolls on the card.
  private function rollChecks():void {
   var chamber:String=HarnessText.DATA.english.st_srp_weapon_chamber;
   // Recoil (115) scales the six kick bounds by max(0, 1 + sum), defaults and defects summed once.
   var rolled:Object=recoilItem(1.8,2,1.4,1.8,1.4,1.8);
   rolled.modifiers=[{props:[{prop_id:115,prop_value:"-0.10"}]}];
   require(stat(rolled,146).value=="1.62…1.8" && stat(rolled,147).value=="0.052","recoil roll scales the upward kick");
   require(stat(rolled,148).value=="-0.36…0.36" && stat(rolled,149).value=="0.147","recoil roll scales the sideways kick");
   rolled.modifiers.push({state:1,props:[{prop_id:115,prop_value:"0.10"}]});
   require(stat(rolled,146).value=="1.8…2","a recoil defect cancels an equal roll");
   rolled.modifiers=[{props:[{prop_id:115,prop_value:"-0.6"}]},{props:[{prop_id:115,prop_value:"-0.6"}]}];
   require(stat(rolled,146).value=="0…0","recoil floored at zero, as the cook");
   // Aim time (102) scales the stock aim-time bar (133): 0.25 s x 0.9.
   var aimed:Object=item(false);aimed.modifiers=[{icon:33,textValue:"Aim time -10%",props:[{prop_id:102,prop_value:"-0.10"}]}];
   shows(aimed,items[0]);
   require(Math.abs(nativeRow(cards[0],"Aim time").value-.225)<1e-6,"aim-time roll shortens the aim-time bar");
   require(Math.abs(nativeRow(cards[0],"Dispersion").value-.2)<1e-6,"an aim-time roll changes no other bar");
   shows(items[1],items[0]);
   require(Math.abs(nativeRow(cards[0],"Aim time").value-.25)<1e-6,"no roll, the weapon's own aim time");
   // Chambering time (160, base x max(0, 1 + sum 119)): only on the weapons that have it.
   require(text(cards[0]).indexOf(chamber)<0 && text(cards[1]).indexOf(chamber)<0,"no chambering row on weapons without one");
   var bolt:Object=boltItem("1.333");
   shows(bolt,items[1]);
   require(text(cards[0]).indexOf(chamber+": 1.33s\n")>=0,"chambering row shows the weapon's own time");
   require(text(cards[1]).indexOf(chamber+": –\n")>=0 && color(cards[1],chamber)==15911311,"a weapon without one shows a dash beside one with it");
   require(color(cards[0],chamber)==15911311,"nothing to compare a chambering time with");
   aligned();
   var quick:Object=boltItem("1.333");quick.modifiers=[{icon:33,textValue:"Chambering -15%",props:[{prop_id:119,prop_value:"-0.15"}]}];
   shows(quick,bolt);
   require(text(cards[0]).indexOf(chamber+": 1.13s\n")>=0,"chambering roll shows the modified time");
   require(color(cards[0],chamber)==8890390 && color(cards[1],chamber)==9834254,"a shorter chambering time is green, the longer red");
   aligned();
   quick.modifiers.push({state:1,icon:36,textValue:"Chambering +10%",props:[{prop_id:119,prop_value:"0.10"}]});
   shows(quick,bolt);
   require(text(cards[0]).indexOf(chamber+": 1.27s\n")>=0,"a chambering defect sums with the roll");
   // Its label fits one line beside the widest chambering value (a rolled 1.333 s), compact and expanded.
   var misses:Array=[];
   for each(var language:String in HarnessText.LANGUAGES){
    Loc.setData(texts(language));
    for each(var expanded:Boolean in [false,true]){
     Scene.isShift=expanded;shows(quick,bolt);
     require(text(cards[0]).indexOf(HarnessText.DATA[language].st_srp_weapon_chamber+": ")>=0,"chambering row: "+language);
     misses=misses.concat(overflows(cards[0],language),overflows(cards[1],language));
    }
   }
   var told:Object={};misses=misses.filter(function(m:String,...rest):Boolean{var again:Boolean=told[m];told[m]=true;return !again;});
   for each(var miss:String in misses)trace("DETAILS OVERFLOW "+miss);
   require(misses.length==0,misses.length+" labels beside a chambering time do not fit one line (DETAILS OVERFLOW lines)");
   Loc.setData(texts("english"));
   shows(items[0],items[1]);
  }
  // Held Alt (docs/notes/alt-weapon-stats.md): the stats as a match has them, with the kit's armour
  // and the skills; a changed one reads "base -> in a match", to the decimal at which they differ.
  private var profile:Object,Kit:Object,Provider:Class;
  private var altView:Object,released:String;
  private function kitFloats(values:Object):Object {var floats:Array=[];for(var i:int=0;i<68;i++)floats.push(values[i]===undefined?0:values[i]);return Kit.parse(floats);}
  private function wearing(items:Array):void {profile.profileItems.dataProvider=new Provider(items);}
  private function piece(slot:int,props:Array):Object {
   var Data:Class=domain.getDefinition("survarium.data.ItemData") as Class;
   var d:Object=new Data();d.slotId=slot;d.isWeapon=slot==7;d.name="piece "+slot;d.properties=[];d.modifiers=[];
   for each(var p:Array in props)d.modifiers.push({props:[{prop_id:p[0],prop_value:p[1]}]});
   return d;
  }
  private function aimTime(d:Object,value:String):Object {for each(var p:Object in d.properties)if(p.prop_id==133)p.prop_value=value;return d;}
  private function nativeText(card:Object,label:String):String {var row:Object=nativeRow(card,label);row.validateNow();return row.textFieldValue.text;}
  private function line(card:Object,start:String):String {
   for each(var l:String in text(card).split("\n"))if(l.indexOf(start)==0)return l;
   throw new Error("missing line: "+start+" in "+text(card));
  }
  private function altChecks():void {
   var Profile:Class=domain.getDefinition("SrvLocalPlayerProfile") as Class;
   profile=new Profile();profile.x=-2000;
   profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
   addChildAt(profile as DisplayObject,0);
   profile.paperDoll.ammoControl=new (domain.getDefinition("survarium.ammo.AmmoShareControll") as Class)();
   Kit=domain.getDefinition("survarium.player.KitModifiers");
   Provider=domain.getDefinition("scaleform.clik.data.DataProvider") as Class;
   var english:Object=HarnessText.DATA.english;
   Scene.isShift=true;
   // No kit, no skills: a weapon's own roll alone. A 0.2 s aim time with -2 % reads 0.19 on the
   // card (two decimals); held Alt, 0.2 -> 0.196, never "x -> x".
   profile.setKitModifiers(kitFloats({}));wearing([]);
   var quick:Object=aimTime(item(false),"0.2");quick.modifiers=[{icon:33,textValue:"Aim time -2%",props:[{prop_id:102,prop_value:"-0.02"}]}];
   Scene.isAlt=false;shows(quick,items[1]);
   require(nativeText(cards[0],"Aim time")=="0.19s","released: the card's own two decimals ("+nativeText(cards[0],"Aim time")+")");
   require(text(cards[0]).indexOf(english.st_srp_weapon_with_kit_skills)<0,"released: no effects line");
   Scene.isAlt=true;shows(quick,items[1]);
   require(nativeText(cards[0],"Aim time")=="0.2 -> 0.196s","aim-time roll on a 0.2 s weapon: "+nativeText(cards[0],"Aim time"));
   require(nativeText(cards[0],"Damage")=="34","a stat nothing changes keeps its single value");
   quick.modifiers[0].props[0].prop_value="-0.04";shows(quick,items[1]);
   require(nativeText(cards[0],"Aim time")=="0.2 -> 0.19s","-4 %: two decimals already differ ("+nativeText(cards[0],"Aim time")+")");
   aimTime(quick,"0.3");quick.modifiers[0].props[0].prop_value="-0.02";shows(quick,items[1]);
   require(nativeText(cards[0],"Aim time")=="0.3 -> 0.29s","-2 % on 0.3 s: rounded, two decimals differ ("+nativeText(cards[0],"Aim time")+")");
   require(line(cards[0],english.st_srp_weapon_with_kit_skills)==english.st_srp_weapon_with_kit_skills,"the effects line under the heading");
   // Skills (the kit's own floats: Steady aim -10 %, reload +12 %, chambering +12 %, weapon change
   // +15 %, hip fire -10 %), gloves (reload +10 %, aim time -6 %), a vest (recoil -4 %, spread -4 %);
   // a weapon's player-modifier line is not armour and adds nothing.
   profile.setKitModifiers(kitFloats({49:-.1,13:.12,21:.12,22:.15,56:-.1}));
   wearing([piece(5,[[114,"0.10"],[102,"-0.06"]]),piece(2,[[115,"-0.04"],[101,"-0.04"]]),piece(7,[[114,"0.50"],[102,"-0.5"]])]);
   var ak:Object=item(false);Scene.isAlt=false;shows(ak,items[0]);released=text(cards[0]);
   Scene.isAlt=true;shows(ak,items[0]);
   require(nativeText(cards[0],"Aim time")=="0.25 -> 0.21s","aim time x max(0, 1 + skills + gloves): "+nativeText(cards[0],"Aim time"));
   require(line(cards[0],"Reload")=="Reload: 3 -> 2.46s","reload / max(0, 1 + 0.12 + 0.10): "+line(cards[0],"Reload"));
   require(line(cards[0],"Draw time")=="Draw time: 0.6 -> 0.52s" && line(cards[0],"Holster time")=="Holster time: 0.6 -> 0.52s","draw and holster / 1.15");
   require(line(cards[0],"Vertical recoil:")=="Vertical recoil: 1.8…2 -> 1.73…1.92","kick x 0.96: "+line(cards[0],"Vertical recoil:"));
   require(line(cards[0],"Hip-fire spread")=="Hip-fire spread: 1 -> 0.86x","hip fire x 0.9 x 0.96: "+line(cards[0],"Hip-fire spread"));
   require(line(cards[0],"Full damage up to")=="Full damage up to: 30m","range: nothing in a kit changes it");
   require(text(cards[1]).indexOf(english.st_srp_weapon_with_kit_skills)>=0,"the compared weapon too");
   require(line(cards[1],"Reload")=="Reload: 10 -> 8.2s","the compared weapon with the same kit: "+line(cards[1],"Reload"));
   require(color(cards[0],"Reload")==8890390,"comparison of the in-match values");
   aligned();
   // An exe that sends no floats: the kit alone, said so.
   profile.setKitModifiers(null);shows(ak,items[0]);
   require(line(cards[0],english.st_srp_weapon_with_kit)==english.st_srp_weapon_with_kit && line(cards[0],"Reload")=="Reload: 3 -> 2.73s","skills unknown: gloves only");
   profile.setKitModifiers(kitFloats({49:-.1,13:.12,21:.12,22:.15,56:-.1}));
   // Every language fits, held Alt and expanded, beside the widest kick values.
   var misses:Array=[],wide:Object=recoilItem(1.98,2.2,1.42,2,1.42,2);
   for each(var language:String in HarnessText.LANGUAGES){
    Loc.setData(texts(language));
    shows(ak,wide);
    misses=misses.concat(overflows(cards[0],language),overflows(cards[1],language));
    require(text(cards[0]).indexOf(HarnessText.DATA[language].st_srp_weapon_with_kit_skills)>=0,"effects line: "+language);
   }
   var told:Object={};misses=misses.filter(function(m:String,...rest):Boolean{var again:Boolean=told[m];told[m]=true;return !again;});
   for each(var miss:String in misses)trace("DETAILS OVERFLOW "+miss);
   require(misses.length==0,misses.length+" labels held Alt do not fit one line (DETAILS OVERFLOW lines)");
   Loc.setData(texts("english"));
   // Pressed and let go while the card is open: the card follows at the next frame.
   Scene.isAlt=false;shows(ak,items[0]);
   require(text(cards[0])==released,"released: the weapon's own card");
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
    for(var id:int=138;id<=160;id++)list.push({prop_id:id,comparable:false});
    // The rolls' own properties, multiplied (modifier_type 1) as the exe tells the movie (mod_type 2).
    for each(id in [102,115,119])list.push({prop_id:id,comparable:false,modifier_type:1});
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
       // A value's right edge (a wide changed value held Alt grows to its left).
       var edge:Function=function(f:TextField):Number{return f.parent.localToGlobal(new Point(f.x+(name=="textFieldValue"?f.width:0),f.y)).x;};
       require(Math.abs(edge(row[name])-edge(original[name]))<.2,"original column alignment: "+name);
       require(Math.abs(row[name].transform.concatenatedMatrix.a-original[name].transform.concatenatedMatrix.a)<.001 && Math.abs(row[name].transform.concatenatedMatrix.d-original[name].transform.concatenatedMatrix.d)<.001,"original text scale: "+name);
      }
      require(row.textFieldLabel.textColor==original.textFieldLabel.textColor,"original label color");
      require([15911311,8890390,9834254].indexOf(row.textFieldValue.textColor)>=0,"original stat palette");
      require(row.textFieldValue.textWidth<row.textFieldValue.width,"value fits row: "+row.textFieldValue.text);
      // The native stride, or whole strides more where the compared card's row wraps.
      if(j+1<child.numChildren && !(child.getChildAt(j+1) is Caption) && row.textFieldLabel.numLines==1){
       var stride:Number=card.itemCompare.diff_2.y-card.itemCompare.diff_1.y,step:Number=(child.getChildAt(j+1).y-child.getChildAt(j).y)/stride;
       require(step>=1-.01 && Math.abs(step-Math.round(step))<.01,"original row spacing");
      }
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
    // A value on the line under its label (a changed range held Alt) leaves the label the row.
    var below:Boolean=value.y>label.y+1;
    var align:String=value.getTextFormat().align,valueStart:Number=value.x;
    if(below)valueStart+=value.width+6;
    else if(align==TextFormatAlign.RIGHT)valueStart+=value.width-value.textWidth-4;
    else if(align==TextFormatAlign.CENTER)valueStart+=(value.width-value.textWidth)/2-2;
    if(label.numLines!=1 || label.x+label.textWidth*(1+FIT_MARGIN)+6>valueStart)misses.push(language+": '"+label.text+"' "+Math.round(label.textWidth)+"/"+Math.round(valueStart-label.x-6)+(label.numLines!=1?" ("+label.numLines+" lines)":"")+(below?" (value below)":""));
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
     require(text(cards[0]).indexOf(HarnessText.DATA.english.st_srp_weapon_shift_hint)>=0 && HarnessText.DATA.english.st_srp_weapon_shift_hint.indexOf("Alt")>=0,"discoverable expanded and Alt views");
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
    if(frame==85){neutral(cards[0]);neutral(cards[1]);require(text(cards[0]).indexOf(HarnessText.DATA.english.st_srp_weapon_shift_hint)>=0,"Ctrl alone retains compact layout");Scene.isCtrl=false;}
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
     rollChecks();
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
     altChecks();
    }
    if(frame==140){Scene.isAlt=true;}
    if(frame==150){require(line(cards[0],"Reload")=="Reload: 3 -> 2.46s" && nativeText(cards[0],"Aim time")=="0.25 -> 0.21s","Alt pressed over an open card");Scene.isAlt=false;}
    if(frame==160){require(text(cards[0])==released && nativeText(cards[0],"Aim time")=="0.25s","Alt let go over an open card");finish(null);}
   }catch(err:Error){finish("frame "+frame+": "+err+" "+err.getStackTrace());}
  }
  private function finish(error:String):void {removeEventListener(Event.ENTER_FRAME,tick);if(!error){Scene.isCtrl=loaderInfo.parameters.neutral=="1";if(loaderInfo.parameters.compact=="1")Scene.isShift=false;Scene.isAlt=loaderInfo.parameters.alt=="1";}trace("DETAILS RESULT "+(error?"fail: "+error:"pass"));}
 }
}
