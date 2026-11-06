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
   var colt:Object=recoilItem(1.3,1.6,.2,.5,.2,.5);
   require(stat(colt,146).value=="1.3…1.6" && stat(colt,147).value=="0.087","Colt upward variability has no floating-point tail");
   require(stat(colt,148).value=="-0.3…0.3" && stat(colt,149).value=="0.122","Colt sideways recoil has no floating-point tail");
   for each(var sample:Array in [[.08699999999999999,"0.087"],[1.23456,"1.23"],[9.99999,"10"],[0,"0"]]){
    for each(var prop:Object in colt.properties)if(prop.prop_id==153)prop.prop_value=String(sample[0]);
    require(stat(colt,153).value==sample[1],"scalar rounds without trailing zeroes: "+sample[1]);
   }
   var tiny:Object=recoilItem(0,.00001,0,.00001,0,.00001);
   require(stat(tiny,148).value=="0…0","rounded recoil range has no negative zero");
   var effects:Object=domain.getDefinition("survarium.player.MatchEffects");
   require(effects.fixed(100,0)=="100","integer precision preserves significant zeroes");
   require(effects.change(.2,.196,2)=="0.2 -> 0.196","match effect retains extra precision for small changes");
   require(effects.fixed(.1234567,6)=="0.123457","match effect rounds at maximum precision");
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
  // The rounds a weapon fires, as the lobby's item descriptors have them: card lines 21 (k_damage x
  // pellets x100), 22 (k_arp x100), 163 (bullet speed), 165 (pellets, the smallshot rounds only) and
  // 174 (pierce-line-1.json: for each weapon that fires it "<weapon dict id>:<rank>:<key>:<percent>
  // ...", the rank the material's place: glass 0 .. stone 6). 3 the AK-74M (damage 34, armour
  // piercing 35) with three rounds whose best of each stat differ: speed 7N10, piercing and material
  // 7N22; 2229 the PKP (the cards' weapons); 2300 an A-545, 2400 an MP-443, 2500 a VSSK stand-in
  // (VPS: damage and piercing x1.2); 2600 a Benelli M4 and 2700 a Remington 870 (buckshot and slugs),
  // 2800 a slug-only shotgun.
  private static const ROUNDS:Array=[
   [901,"7N22",100,120,880,0,"3:5:st_srp_pierce_concrete:81 2300:5:st_srp_pierce_concrete:52"],
   [902,"7N10",100,110,920,0,"3:5:st_srp_pierce_concrete:68 2300:5:st_srp_pierce_concrete:35"],
   [905,"7N6M",100,100,900,0,"3:5:st_srp_pierce_concrete:52"],
   [903,"7N1",100,100,830,0,"2229:5:st_srp_pierce_concrete:100"],
   [904,"Para",100,100,360,0,"2400:3:st_srp_pierce_tinplate:89"],
   [906,"12.7x54 VPS",120,120,320,0,"2500:6:st_srp_pierce_stone:36"],
   [907,"12.7x54 PT",100,100,320,0,"2500:5:st_srp_pierce_concrete:100"],
   [908,"12mm buck",200,60,400,8,"2600:0:st_srp_pierce_glass:100 2700:0:st_srp_pierce_glass:100"],
   [910,"12mm buck2",240,50,380,10,"2600:0:st_srp_pierce_glass:100"],
   [909,"12mm slug",120,100,450,0,"2600:2:st_srp_pierce_plywood:19 2700:2:st_srp_pierce_plywood:5 2800:2:st_srp_pierce_plywood:30"]];
  private static const ROUND_PAIRS:Array=[[3,901],[902,3],[3,905],[2300,901],[2300,902],[2229,903],[2400,904],[2500,906],[2500,907],
   [2600,908],[2600,910],[2600,909],[2700,908],[2700,909],[2800,909]].map(function(p:Array,...rest):Object{return {first_item_dict_id:p[0],second_item_dict_id:p[1]};});
  private function roundDescriptors():Array {
   var out:Array=[];
   for each(var r:Array in ROUNDS){
    var props:Array=[{prop_id:21,prop_value:String(r[2])},{prop_id:22,prop_value:String(r[3])},{prop_id:163,prop_value:String(r[4])},{prop_id:174,prop_value:r[6]}];
    if(r[5])props.push({prop_id:165,prop_value:String(r[5])});
    out.push({dictId:r[0],name:r[1],item_properties:props});
   }
   return out;
  }
  private function weapon(dictId:int,name:String):Object {var d:Object=item(false);d.dictId=dictId;d.name=name;return d;}
  private function round(name:String,line:String,ap:String):Object {
   var Data:Class=domain.getDefinition("survarium.data.ItemData") as Class;
   var d:Object=new Data();d.name=name;d.category=9;d.superCategory=3;d.isWeapon=false;d.itemLevel=10;d.modifiers=[];d.modifiersDefault=[];
   d.properties=[{prop_id:21,prop_value:"100"},{prop_id:22,prop_value:ap},{prop_id:163,prop_value:"800"},{prop_id:16,prop_value:"30"},{prop_id:15,prop_value:"0.3"}];
   if(line)d.properties.push({prop_id:174,prop_value:line});
   return d;
  }
  // "Concrete · 52%" in the current language.
  private function pierceText(t:Object,key:String,percent:int):String {
   return t.st_srp_pierce_value.split("%1$s").join(t["st_srp_pierce_"+key]).split("%2$s").join(t.st_srp_unit_percent.split("%1$s").join(String(percent)));
  }
  private function mps(t:Object,n:int):String {return t.st_srp_unit_mps.split("%1$s").join(String(n));}
  // The first detail row's label field labelled `text`, in global coordinates (its x).
  private function label(card:Object,text:String):Point {
   var view:Object=detailsView(card);
   for(var i:int=0;i<view.numChildren;i++){var e:Object=view.getChildAt(i);
    if(!(e is Caption) && e.indicator.textFieldLabel.text==text){var f:TextField=e.indicator.textFieldLabel;return f.parent.localToGlobal(new Point(f.x,f.y));}}
   throw new Error("missing stat: "+text);
  }
  // The value colours of every detail row labelled `label`.
  private function colours(card:Object,label:String):Array {
   var view:Object=detailsView(card),out:Array=[];
   for(var i:int=0;i<view.numChildren;i++){var e:Object=view.getChildAt(i);if(!(e is Caption) && e.indicator.textFieldLabel.text==label)out.push(e.indicator.textFieldValue.textColor);}
   return out;
  }
  private function both(label:String):String {return [color(cards[0],label),color(cards[1],label)].join();}
  private function nativeBoth(label:String):String {return [nativeField(cards[0],label).textColor,nativeField(cards[1],label).textColor].join();}
  private function pierceChecks():void {
   Scene.isShift=false;Scene.isAlt=false;
   var M:String="Pierces",S:String="Bullet speed";
   for each(var language:String in HarnessText.LANGUAGES){
    Loc.setData(texts(language));
    var t:Object=HarnessText.DATA[language];
    // Any weapon but a shotgun of both kinds: each stat its best round's, from different rounds;
    // held Shift the same rows, no round's own.
    for each(var shift:Boolean in [false,true]){
     Scene.isShift=shift;shows(item(false),item(true));
     var card:String=text(cards[0]);
     require(card.indexOf(t.st_srp_inv_material_pierce+": "+pierceText(t,"concrete",81)+"\n")>=0,"the best material (7N22): "+language+" "+shift+" "+card);
     require(card.indexOf(t.st_srp_inv_bullet_speed+": "+mps(t,920)+"\n")>=0,"the best speed (7N10): "+language+" "+shift);
     require(card.indexOf("•")<0,"no round's own rows: "+language+" "+shift);
     require(overflows(cards[0],language).length==0 && overflows(cards[1],language).length==0,"pierce line fits: "+language+" "+overflows(cards[0],language));
     aligned();
    }
    Scene.isShift=false;
    // A shotgun of buckshot and slugs: each of the four stats a header over its best Buckshot and
    // best Slug (damage 34 x 2.4 = 82 of buck2 over 34 x 2 of buck; piercing 35 x 0.6 = 21 of buck).
    shows(weapon(2600,"Benelli M4"),weapon(2700,"Remington 870"));
    card=text(cards[0]);
    var B:String="• "+t.st_srp_ammo_buckshot,L:String="• "+t.st_srp_ammo_slug;
    var kinds:String="Minimum damage: 60%\nDamage: \n"+B+": 82\n"+L+": 41\nArmor penetration: \n"+B+": 21\n"+L+": 35\n"+
     t.st_srp_inv_bullet_speed+": \n"+B+": "+mps(t,400)+"\n"+L+": "+mps(t,450)+"\n"+
     t.st_srp_inv_material_pierce+": \n"+B+": "+pierceText(t,"glass",100)+"\n"+L+": "+pierceText(t,"plywood",19)+"\n";
    require(card.indexOf(kinds.substr(kinds.indexOf("\n")+1))>0,"a shotgun's stats by kind: "+language+" "+card.split("\n").join("|"));
    require(label(cards[0],L).x>label(cards[0],t.st_srp_inv_material_pierce).x+8,"a kind steps in under its stat: "+language);
    require(overflows(cards[0],language).length==0 && overflows(cards[1],language).length==0,"kind rows fit: "+language+" "+overflows(cards[0],language));
    aligned();
   }
   Loc.setData(texts("english"));
   // The native bars take the best round: the AK-74M's armour piercing 35 x 1.2 (7N22), its damage
   // x1 (every round); the VSSK's damage 34 x 1.2 and piercing 35 x 1.2 (VPS, PT x1); a shotgun of
   // both kinds its best slug's (34 x 1.2, piercing x1).
   shows(item(false),weapon(2500,"VSSK"));
   require(nativeText(cards[0],"Damage")=="34" && nativeText(cards[0],"Armor penetration")=="42","best-round bars: "+nativeText(cards[0],"Damage")+" "+nativeText(cards[0],"Armor penetration"));
   require(nativeText(cards[1],"Damage")=="41" && nativeText(cards[1],"Armor penetration")=="42","the best round's damage, whole (34 x 1.2): "+nativeText(cards[1],"Damage"));
   shows(weapon(2600,"Benelli M4"),item(false));
   require(nativeText(cards[0],"Damage")=="41" && nativeText(cards[0],"Armor penetration")=="35","a shotgun's slug bars: "+nativeText(cards[0],"Damage"));
   // Shotgun against shotgun: each kind against the same kind (the slugs: plywood 19 % against 5 %).
   shows(weapon(2600,"Benelli M4"),weapon(2700,"Remington 870"));
   require(colours(cards[0],"• Slug").join()==[NEUTRAL,NEUTRAL,NEUTRAL,GREEN].join() && colours(cards[1],"• Slug").join()==[NEUTRAL,NEUTRAL,NEUTRAL,RED].join(),
    "slug against slug: "+colours(cards[0],"• Slug")+" / "+colours(cards[1],"• Slug"));
   require(colours(cards[0],"• Buckshot").join()==[GREEN,NEUTRAL,NEUTRAL,NEUTRAL].join() && colours(cards[1],"• Buckshot").join()==[RED,NEUTRAL,NEUTRAL,NEUTRAL].join(),
    "buckshot against buckshot: "+colours(cards[0],"• Buckshot")+" / "+colours(cards[1],"• Buckshot"));
   require(colours(cards[0],"Pierces").join()==[NEUTRAL].join() && colours(cards[0],"Damage").join()==[NEUTRAL].join(),"a header has no value and no colour");
   // Shotgun against any other gun (an MP-443 stand-in, Para: 360 m/s, Tinplate · 89%): a gun of
   // slugs, its best slug's rows, no kinds shown (the slug's 450 m/s green, Plywood · 19% red), the
   // other gun's best against them.
   shows(weapon(2600,"Benelli M4"),weapon(2400,"MP-443"));
   require(text(cards[0]).indexOf("•")<0 && text(cards[1]).indexOf("•")<0 && text(cards[1]).indexOf("–")<0,"no kinds, nothing padded");
   require(line(cards[0],S)=="Bullet speed: 450 m/s" && line(cards[0],M)=="Pierces: Plywood · 19%","the shotgun's slug rows: "+line(cards[0],S)+" "+line(cards[0],M));
   require(both(S)==[GREEN,RED].join() && both(M)==[RED,GREEN].join(),"slug against the gun's best: "+both(S)+" "+both(M));
   require(text(cards[0]).indexOf("Damage: ")<0,"its damage and piercing the bars only, as any gun's");
   // Alone (no compared card), a shotgun shows its kinds.
   require(Details.read(weapon(2600,"Benelli M4")).rows(false,null).some(function(r:Object,...rest):Boolean{return r.label=="• Buckshot";}),"a shotgun alone: its kinds");
   // The bars compare as any: the shotgun's slug damage 41 and piercing 35 against the AK-74M's best round's 34 and 42.
   shows(weapon(2600,"Benelli M4"),item(false));
   require(nativeBoth("Armor penetration")==[RED,GREEN].join() && nativeBoth("Damage")==[GREEN,RED].join(),"a shotgun's bars compare: "+nativeBoth("Armor penetration"));
   // A shotgun of one kind is any weapon: its best.
   shows(weapon(2800,"Slugger"),weapon(2700,"Remington 870"));
   require(line(cards[0],M)=="Pierces: Plywood · 30%" && text(cards[0]).indexOf("•")<0,"one kind: the best");
   // Held Shift a shotgun the same.
   Scene.isShift=true;shows(weapon(2600,"Benelli M4"),weapon(2700,"Remington 870"));
   require(colours(cards[0],"• Slug").length==4,"held Shift: the kinds");
   Scene.isShift=false;
   // The harder material wins over more damage kept: the VSSK's VPS (stone, 36 %) over PT (concrete, 100 %).
   shows(weapon(2500,"VSSK"),item(false));
   require(line(cards[0],M)=="Pierces: Stone · 36%","the harder material is the best");
   require(both(M)==[GREEN,RED].join(),"harder is better: "+both(M));
   // The same material: more damage kept is better (81 against the A-545's 52).
   shows(item(false),weapon(2300,"A-545"));
   require(line(cards[1],M)=="Pierces: Concrete · 52%" && both(M)==[GREEN,RED].join(),"more damage kept is better: "+both(M));
   // Equal: neutral. Ctrl: neutral.
   shows(item(false),weapon(3,"AK-74M copy"));
   require(both(M)==[NEUTRAL,NEUTRAL].join() && both(S)==[NEUTRAL,NEUTRAL].join(),"equal stays neutral");
   Scene.isCtrl=true;shows(item(false),weapon(2400,"MP-443"));
   require(both(M)==[NEUTRAL,NEUTRAL].join() && both(S)==[NEUTRAL,NEUTRAL].join(),"Ctrl: neutral");
   Scene.isCtrl=false;
   // Best against best: the fastest round, the hardest material, the best-round bars.
   shows(item(false),weapon(2400,"MP-443"));
   require(both(S)==[GREEN,RED].join() && both(M)==[GREEN,RED].join(),"faster and harder are better");
   require(nativeBoth("Armor penetration")==[GREEN,RED].join(),"best-round armour piercing compares: "+nativeBoth("Armor penetration"));
   // A weapon no round speaks for beside one: a dash, neutral; two such: no rows.
   shows(weapon(4444,"no rounds"),item(false));
   require(line(cards[0],M)=="Pierces: –" && line(cards[0],S)=="Bullet speed: –","a dash beside a weapon with the rows");
   require(both(M)==[NEUTRAL,NEUTRAL].join() && both(S)==[NEUTRAL,NEUTRAL].join(),"missing on one side: neutral");aligned();
   shows(weapon(4444,"no rounds"),weapon(4445,"none either"));
   require(text(cards[0]).indexOf(M)<0 && text(cards[0]).indexOf(S)<0,"no rows without the data");
   // The ammo card shows no Pierces row (a weapon's and a round's together).
   shows(round("7N22",ROUNDS[0][6],"120"),round("7N10",ROUNDS[1][6],"110"));
   var Ammo:Object=domain.getDefinition("survarium.item.AmmoDetails");
   for each(var row:Object in Ammo.read(cards[0].data).rows(true,Ammo.read(cards[1].data)))require(row.id!=174 && row.label!=M,"no row on a round's card");
   // Not comparable: no bar.
   shows(item(false),item(true));
   for(var i:int=1;i<=9;i++){var bar:Object=cards[0].itemCompare.getChildByName("diff_"+i);require(!bar || !bar.visible || bar.indicator.label!=M,"no pierce bar");}
   shows(items[0],items[1]);
  }
  // For a capture (-P pierce=compare|shotgun|details): the cards of a before|after pair, no checks.
  private function pierceCapture(what:String):void {
   Scene.isShift=what=="details";
   if(what=="shotgun")shows(weapon(2600,"Benelli M4"),item(false));
   else shows(item(false),item(true));
   removeEventListener(Event.ENTER_FRAME,tick);trace("DETAILS RESULT capture "+what+" heights "+[cards[0].height/cards[0].scaleY,cards[1].height/cards[1].scaleY].join());
  }
  private function tacticalItem():Object {var d:Object=item(false);d.properties.push({prop_id:162,prop_value:String(4/3)});return d;}
  private function tacticalChecks():void {
   var gun:Object=tacticalItem(),plain:Object=item(true);
   require(stat(gun,138).value=="4s" && stat(gun,162).value=="3s","empty and tactical times derive from the clip ratio");
   require(stat(plain,162)==null,"ordinary weapon alone has no tactical row");
   plain.properties.push({prop_id:161,prop_value:"42"});
   require(stat(plain,162)==null,"kill-feed icon metadata is not a reload ratio");
   gun.modifiers=[{icon:33,textValue:"Reload +25%",props:[{prop_id:114,prop_value:"0.25"}]}];
   require(stat(gun,138).value=="3s" && stat(gun,162).value=="2.25s","weapon reload roll scales both reloads");
   shows(gun,tacticalItem());
   require(color(cards[0],"Tactical reload")==GREEN && color(cards[1],"Tactical reload")==RED,"two tactical times compare numerically");
   aligned();
   gun.modifiers=[];
   for each(var language:String in HarnessText.LANGUAGES){
    Loc.setData(texts(language));
    var label:String=HarnessText.DATA[language].st_srp_weapon_reload_tactical;
    for each(var expanded:Boolean in [false,true]){
     Scene.isShift=expanded;
     shows(gun,plain);
     require(line(cards[1],label)==label+": -","missing tactical row on right: "+language);
     require(color(cards[0],label)==NEUTRAL && color(cards[1],label)==NEUTRAL,"missing value has no comparison colour");
     aligned();
     shows(plain,gun);
     require(line(cards[0],label)==label+": -","missing tactical row on left: "+language);aligned();
     shows(plain,item(true));
     require(text(cards[0]).indexOf(label)<0 && text(cards[1]).indexOf(label)<0,"neither weapon has tactical reload");
    }
   }
   Loc.setData(texts("english"));Scene.isShift=true;shows(items[0],items[1]);
  }
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
  // A changed value's colours: [its original (first character), the arrow, its current (last character)].
  private function changeColours(field:TextField):Array {
   var t:String=field.text,at:int=t.lastIndexOf(" -> ");
   require(at>0,"a changed value: "+t);
   // RGB only: a colour set with textColor reads back with its alpha byte in Ruffle.
   var rgb:Function=function(begin:int):uint{return uint(field.getTextFormat(begin,begin+1).color)&0xFFFFFF;};
   return [rgb(0),rgb(at+1),rgb(t.length-1)];
  }
  private function detailField(card:Object,label:String):TextField {
   var view:Object=detailsView(card);
   for(var i:int=0;i<view.numChildren;i++){var e:Object=view.getChildAt(i);if(!(e is Caption) && e.indicator.textFieldLabel.text==label)return e.indicator.textFieldValue;}
   throw new Error("missing stat: "+label);
  }
  private function nativeField(card:Object,label:String):TextField {var row:Object=nativeRow(card,label);row.validateNow();return row.textFieldValue;}
  private static const NEUTRAL:uint=15911311,GREEN:uint=8890390,RED:uint=9834254;
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
   require(changeColours(nativeField(cards[0],"Aim time")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"the roll's shorter aim time: green");
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
   // The current part green when better for the player, red when worse; the original and the arrow neutral.
   require(changeColours(detailField(cards[0],"Reload")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"a shorter reload: green ("+changeColours(detailField(cards[0],"Reload"))+")");
   require(changeColours(nativeField(cards[0],"Aim time")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"a shorter aim time on the bar's row: green");
   require(changeColours(detailField(cards[0],"Vertical recoil")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"a smaller kick: green");
   require(changeColours(detailField(cards[0],"Hip-fire spread")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"less hip-fire spread: green");
   require(color(cards[0],"Full damage up to")==NEUTRAL,"an unchanged stat keeps its usual colour");
   Scene.isCtrl=true;shows(ak,items[0]);
   require(changeColours(detailField(cards[0],"Reload")).join()==[NEUTRAL,NEUTRAL,NEUTRAL].join(),"Ctrl: neutral");
   Scene.isCtrl=false;
   // A worse kit: gloves with a reload defect (-30 %) and a slower aim (+20 %), no skills.
   profile.setKitModifiers(kitFloats({}));wearing([piece(5,[[114,"-0.30"],[102,"0.20"]])]);shows(ak,items[0]);
   require(line(cards[0],"Reload")=="Reload: 3 -> 4.29s","reload / 0.7: "+line(cards[0],"Reload"));
   require(changeColours(detailField(cards[0],"Reload")).join()==[NEUTRAL,NEUTRAL,RED].join(),"a longer reload: red");
   require(nativeText(cards[0],"Aim time")=="0.25 -> 0.3s" && changeColours(nativeField(cards[0],"Aim time")).join()==[NEUTRAL,NEUTRAL,RED].join(),"a slower aim on the bar's row: red");
   profile.setKitModifiers(kitFloats({49:-.1,13:.12,21:.12,22:.15,56:-.1}));
   wearing([piece(5,[[114,"0.10"],[102,"-0.06"]]),piece(2,[[115,"-0.04"],[101,"-0.04"]]),piece(7,[[114,"0.50"],[102,"-0.5"]])]);shows(ak,items[0]);
   require(color(cards[0],"Reload")==NEUTRAL,"comparison of the in-match values: the change's colours");
   aligned();
   // An exe that sends no floats: the kit alone, said so.
   profile.setKitModifiers(null);shows(ak,items[0]);
   require(line(cards[0],english.st_srp_weapon_with_kit)==english.st_srp_weapon_with_kit && line(cards[0],"Reload")=="Reload: 3 -> 2.73s","skills unknown: gloves only");
   profile.setKitModifiers(kitFloats({49:-.1,13:.12,21:.12,22:.15,56:-.1}));
   // Every language fits, held Alt and expanded, beside the widest kick values.
   var tacticalGun:Object=tacticalItem();shows(tacticalGun,items[0]);
   require(line(cards[0],"Tactical reload")=="Tactical reload: 3 -> 2.46s","kit and skills scale tactical reload");
   require(line(cards[0],"Reload (empty)")=="Reload (empty): 4 -> 3.28s","kit and skills scale empty reload");
   require(line(cards[1],"Tactical reload")=="Tactical reload: -","Alt preserves the missing row");aligned();
   Scene.isCtrl=true;shows(tacticalGun,items[0]);
   require(changeColours(detailField(cards[0],"Tactical reload")).join()==[NEUTRAL,NEUTRAL,NEUTRAL].join(),"Ctrl suppresses tactical change colours");
   Scene.isCtrl=false;
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
  // Held Alt over a medicine (docs/notes/alt-weapon-stats.md, "Medicines"): its card's lines after
  // the medical skills, the ranks packed in kit float 43, Medical Training in float 57.
  // The live cards (scripts/content-edit/medicines.tsv, the painkiller's and antitoxin's configs).
  private static const MEDICINES:Object={
   bandages:[[19,"1.0"],[18,"1.0"],[135,"100"],[136,"35"],[137,"35"],[15,"0.1"]],
   medkit:[[19,"2.0"],[18,"3.0"],[135,"100"],[136,"100"],[137,"40"],[15,"0.2"]],
   military_medkit:[[19,"2.0"],[18,"1.0"],[135,"100"],[136,"60"],[137,"50"],[15,"0.3"]],
   blackmarket_medkit:[[19,"2.0"],[18,"2.0"],[135,"0"],[136,"100"],[137,"100"],[15,"0.3"]],
   science_medkit:[[19,"2.0"],[18,"3.0"],[135,"100"],[136,"100"],[137,"50"],[13,"100"],[15,"0.3"]],
   antitoxin:[[13,"50"],[19,"1.0"],[18,"10.0"],[15,"0.1"]],
   painkiller:[[17,"10"],[19,"1.0"],[18,"10.0"],[15,"0.1"]]
  };
  private static const MEDICINE_LABELS:Object={13:"Intoxication",17:"Pain protection",18:"Duration",19:"Activation",135:"Healing: arms / legs",136:"Healing: body / head",137:"Pain relief",15:"Weight"};
  private function medicine(name:String):Object {
   var Data:Class=domain.getDefinition("survarium.data.ItemData") as Class;
   var d:Object=new Data();d.name=name;d.category=10;d.superCategory=domain.getDefinition("survarium.item.ItemFactory").getCategory(10);d.descriptor=100;d.modifiers=[];d.properties=[];
   for each(var p:Array in MEDICINES[name])d.properties.push({prop_id:p[0],prop_value:p[1]});
   return d;
  }
  // Each line of a medicine's card: "label=value", in the card's order.
  private function medicineLines(card:Object,d:Object):Array {
   var out:Array=[];
   for each(var p:Array in MEDICINES[d.name])out.push(MEDICINE_LABELS[p[0]]+"="+nativeText(card,MEDICINE_LABELS[p[0]]));
   return out;
  }
  // Every bar line a card shows: "label=value", in its order.
  private function cardLines(card:Object):Array {
   var out:Array=[];
   for(var i:int=1;i<=9;i++){var row:Object=card.itemCompare.getChildByName("diff_"+i);if(row && row.visible){row.indicator.validateNow();out.push(row.indicator.label+"="+row.indicator.textFieldValue.text);}}
   return out;
  }
  // A bar line's label that runs into its value, or takes two lines.
  private function barOverflows(card:Object,language:String):Array {
   var misses:Array=[];
   for(var i:int=1;i<=9;i++){
    var row:Object=card.itemCompare.getChildByName("diff_"+i);
    if(!row || !row.visible)continue;
    row.indicator.validateNow();
    var label:TextField=row.indicator.textFieldLabel,value:TextField=row.indicator.textFieldValue;
    var align:String=value.getTextFormat().align,valueStart:Number=value.x;
    if(align==TextFormatAlign.RIGHT)valueStart+=value.width-value.textWidth-4;
    else if(align==TextFormatAlign.CENTER)valueStart+=(value.width-value.textWidth)/2-2;
    if(label.numLines!=1 || label.x+label.textWidth*(1+FIT_MARGIN)+6>valueStart)misses.push(language+": '"+label.text+"' "+Math.round(label.textWidth)+"/"+Math.round(valueStart-label.x-6)+" beside '"+value.text+"'");
   }
   return misses;
  }
  // The medical skills as srp packs them (bit offsets 0, 3, 9; medical-skills.md), Medical
  // Training also its +0.08 a rank of drug_amount (57).
  private function medicalKit(painkiller:int,bandaging:int,training:int):Object {return kitFloats({43:painkiller|bandaging<<3|training<<9,57:training*.08});}
  private function medicineChecks():void {
   Scene.isShift=false;
   var names:Array=["bandages","medkit","military_medkit","blackmarket_medkit","science_medkit","antitoxin","painkiller"];
   var other:Object=medicine("medkit"),released:Object={},name:String,d:Object;
   // Released, and held with no medical skill: the card's own numbers (the gloves altChecks
   // left on change no medicine line).
   profile.setKitModifiers(kitFloats({}));
   for each(name in names){
    Scene.isAlt=false;d=medicine(name);shows(d,other);released[name]=medicineLines(cards[0],d).join();
    require(released[name]==MEDICINES[name].map(function(p:Array,...rest):String{return MEDICINE_LABELS[p[0]]+"="+Number(p[1])+(p[0]==15?"kg":p[0]==13?"":p[0]==18||p[0]==19?"s":"%");}).join(),"released "+name+": "+released[name]);
    Scene.isAlt=true;shows(d,other);
    require(medicineLines(cards[0],d).join()==released[name],"Alt, no medical skills: "+name+" "+medicineLines(cards[0],d).join());
   }
   // Each skill alone at its most ranks, then all three; the expected numbers from the spec's
   // rules (medical-skills.md): Potent Painkiller (3) protection 10 % -> 13 % (+1 point a rank), +4.5 s; Fast
   // Bandaging (2) use time x (1 - 2 x 0.05) on a medicine that heals; Medical Training (3) each
   // healing line x 1.24, a pool at most 100 %.
   var painkiller:Object={painkiller:"Pain protection=10 -> 13%,Activation=1s,Duration=10 -> 14.5s,Weight=0.1kg"};
   var bandaging:Object={
    bandages:"Activation=1 -> 0.9s",medkit:"Activation=2 -> 1.8s",military_medkit:"Activation=2 -> 1.8s",
    blackmarket_medkit:"Activation=2 -> 1.8s",science_medkit:"Activation=2 -> 1.8s",antitoxin:"Activation=1 -> 0.9s"
   };
   var training:Object={
    bandages:["Healing: body / head=35 -> 43.4%","Pain relief=35 -> 43.4%"],medkit:["Pain relief=40 -> 49.6%"],
    military_medkit:["Healing: body / head=60 -> 74.4%","Pain relief=50 -> 62%"],science_medkit:["Pain relief=50 -> 62%"],
    antitoxin:["Intoxication=50 -> 62"]
   };
   // The lines a rule changes, the others as released.
   var expect:Function=function(name:String,changes:Array):String{
    var lines:Array=released[name].split(",");
    for each(var change:String in changes){var label:String=change.split("=")[0];for(var i:int=0;i<lines.length;i++)if(lines[i].split("=")[0]==label)lines[i]=change;}
    return lines.join();
   };
   var cases:Array=[[3,0,0],[0,2,0],[0,0,3],[3,2,3]];
   for each(var ranks:Array in cases){
    profile.setKitModifiers(medicalKit(ranks[0],ranks[1],ranks[2]));
    for each(name in names){
     var changes:Array=[];
     if(ranks[0]&&name=="painkiller")changes=changes.concat(painkiller.painkiller.split(","));
     if(ranks[1]&&bandaging[name])changes.push(bandaging[name]);
     if(ranks[2]&&training[name])changes=changes.concat(training[name]);
     d=medicine(name);Scene.isAlt=true;shows(d,other);
     var got:String=medicineLines(cards[0],d).join();
     require(got==expect(name,changes),"ranks "+ranks+" "+name+": "+got+" (want "+expect(name,changes)+")");
     Scene.isAlt=false;shows(d,other);
     require(medicineLines(cards[0],d).join()==released[name],"released with ranks "+ranks+": "+name);
    }
   }
   // Better is green: a shorter use, more healing, more protection.
   profile.setKitModifiers(medicalKit(3,2,3));Scene.isAlt=true;
   d=medicine("bandages");shows(d,other);
   require(changeColours(nativeField(cards[0],"Activation")).join()==[NEUTRAL,NEUTRAL,GREEN].join() && changeColours(nativeField(cards[0],"Pain relief")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"bandages: shorter and more is green");
   d=medicine("painkiller");shows(d,other);
   require(changeColours(nativeField(cards[0],"Pain protection")).join()==[NEUTRAL,NEUTRAL,GREEN].join() && changeColours(nativeField(cards[0],"Duration")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"painkiller: more and longer is green");
   // Packed ranks the exe refuses (Potent Painkiller 6 of 3; or a valid 43 beside Numb Legs 3 of
   // 2 in float 45) are no medical skill at all, as in a match.
   profile.setKitModifiers(kitFloats({43:6}));d=medicine("painkiller");shows(d,other);
   require(medicineLines(cards[0],d).join()==released.painkiller,"refused ranks change nothing");
   profile.setKitModifiers(kitFloats({43:3,45:3}));d=medicine("painkiller");shows(d,other);
   require(medicineLines(cards[0],d).join()==released.painkiller,"refused combo ranks change nothing");
   // Unflinching's three bits at 4: rank 5 of 4 is refused; every combo skill at its most (74) is
   // a valid set beside Potent Painkiller 3.
   profile.setKitModifiers(kitFloats({43:3,45:5<<4}));d=medicine("painkiller");shows(d,other);
   require(medicineLines(cards[0],d).join()==released.painkiller,"Unflinching 5 refused");
   profile.setKitModifiers(kitFloats({43:3,45:74}));d=medicine("painkiller");shows(d,other);
   require(medicineLines(cards[0],d).join()==expect("painkiller",painkiller.painkiller.split(",")),"full combo ranks valid: "+medicineLines(cards[0],d).join());
   // Concentration's three bits at 7: rank 5 of 4 is refused, and so is Concentration beside
   // Unflinching (they exclude each other); its full set (2 + 8 + 4 x 128 = 522) is valid.
   for each(var refused:int in [5<<7,(1<<4)|(1<<7),(4<<4)|(4<<7)]) {
    profile.setKitModifiers(kitFloats({43:3,45:refused}));d=medicine("painkiller");shows(d,other);
    require(medicineLines(cards[0],d).join()==released.painkiller,"combo "+refused+" refused");
   }
   profile.setKitModifiers(kitFloats({43:3,45:522}));d=medicine("painkiller");shows(d,other);
   require(medicineLines(cards[0],d).join()==expect("painkiller",painkiller.painkiller.split(",")),"Concentration set valid: "+medicineLines(cards[0],d).join());
   // The skills that act while a painkiller is active or as a medicine heals (float 45: Numb Legs,
   // Take Cover, Unflinching, Concentration): lines below the card's own, in % of the game's own,
   // each as medical_skills.rs applies it; none without a rank, none once Alt is let go.
   var E:Object=HarnessText.DATA.english;
   var sprint:String=E.st_srp_medicine_sprint_stamina,slow:String=E.st_srp_medicine_hit_slow,ambient:String=E.st_srp_medicine_ambient_sounds;
   var enemy:String=E.st_srp_medicine_enemy_sounds,crouched:String=E.st_srp_medicine_crouched_healing;
   // [float 45, the painkiller's lines, every other medicine's (each heals)].
   var combos:Array=[
    [0,[],[]],
    [1,[sprint+"=100 -> 80%"],[]],[2,[sprint+"=100 -> 60%"],[]],
    [1<<2,[],[crouched+"=100 -> 110%"]],[2<<2,[],[crouched+"=100 -> 120%"]],
    [1<<4,[slow+"=100 -> 90%"],[]],[3<<4,[slow+"=100 -> 70%"],[]],[4<<4,[slow+"=100 -> 60%"],[]],
    [1<<7,[ambient+"=100 -> 90%",enemy+"=100 -> 110%"],[]],[4<<7,[ambient+"=100 -> 60%",enemy+"=100 -> 140%"],[]],
    [74,[sprint+"=100 -> 60%",slow+"=100 -> 60%"],[crouched+"=100 -> 120%"]],
    [522,[sprint+"=100 -> 60%",ambient+"=100 -> 60%",enemy+"=100 -> 140%"],[crouched+"=100 -> 120%"]]
   ];
   for each(var combo:Array in combos){
    profile.setKitModifiers(kitFloats({43:3|2<<3,45:combo[0]}));
    for each(name in names){
     d=medicine(name);Scene.isAlt=true;shows(d,other);
     var own:Array=medicineLines(cards[0],d),wanted:String=own.concat(name=="painkiller"?combo[1]:combo[2]).join();
     require(cardLines(cards[0]).join()==wanted,"combo "+combo[0]+" "+name+": "+cardLines(cards[0]).join()+" (want "+wanted+")");
     Scene.isAlt=false;shows(d,other);
     require(cardLines(cards[0]).join()==released[name],"released with combo "+combo[0]+": "+name+" "+cardLines(cards[0]).join());
    }
   }
   // Refused ranks (Unflinching beside Concentration) add no line, as in a match.
   profile.setKitModifiers(kitFloats({45:(4<<4)|(4<<7)}));d=medicine("painkiller");Scene.isAlt=true;shows(d,other);
   require(cardLines(cards[0]).join()==released.painkiller,"refused combo: no skill line");
   // Better is green: less stamina and slow, quieter surroundings, louder enemies, more healing.
   profile.setKitModifiers(kitFloats({45:522}));shows(d,other);
   for each(var label:String in [sprint,ambient,enemy])require(changeColours(nativeField(cards[0],label)).join()==[NEUTRAL,NEUTRAL,GREEN].join(),label+" is green");
   profile.setKitModifiers(kitFloats({45:74}));shows(d,other);
   require(changeColours(nativeField(cards[0],slow)).join()==[NEUTRAL,NEUTRAL,GREEN].join(),slow+" is green");
   d=medicine("bandages");shows(d,other);
   require(changeColours(nativeField(cards[0],crouched)).join()==[NEUTRAL,NEUTRAL,GREEN].join(),crouched+" is green");
   // Every language's line fits beside its value.
   var misses:Array=[];
   for each(var language:String in HarnessText.LANGUAGES){
    Loc.setData(texts(language));
    for each(var floats:int in [74,522]){
     profile.setKitModifiers(kitFloats({45:floats}));
     shows(medicine("painkiller"),medicine("bandages"));misses=misses.concat(barOverflows(cards[0],language),barOverflows(cards[1],language));
    }
   }
   Loc.setData(texts("english"));
   for each(var miss:String in misses)trace("DETAILS OVERFLOW "+miss);
   require(misses.length==0,misses.length+" medicine skill lines do not fit (DETAILS OVERFLOW lines)");
   // An exe that sends no floats: no skill known, the card's own numbers.
   profile.setKitModifiers(null);d=medicine("bandages");shows(d,other);
   require(medicineLines(cards[0],d).join()==released.bandages,"no floats: the card's own numbers");
   require(cardLines(cards[0]).join()==released.bandages,"no floats: no skill line");
   // altChecks' skills again, for the open-card checks that follow.
   profile.setKitModifiers(kitFloats({49:-.1,13:.12,21:.12,22:.15,56:-.1}));
   Scene.isAlt=false;Scene.isShift=true;shows(item(false),items[0]);
  }
  // Held Alt over a grenade or a trap (docs/notes/alt-weapon-stats.md, "Grenades and traps"): the
  // live cards (throwable-cards-1, throwables.tsv), Grenadier (skill 203) in kit float 27, device
  // damage (11) and splash radius (12) as the exe applies them, though nothing gives them yet.
  private static const THROWABLES:Object={
   rgd5:[24,[[1,"130"],[3,"15"],[132,"10"],[167,"10"],[23,"10"],[15,"0.3"],[166,"2"],[168,"4"],[169,"17"]]],
   smoke:[24,[[15,"0.4"],[168,"4"],[169,"17"]]],
   trap:[11,[[1,"60"],[3,"100"],[170,"5"],[15,"0.8"]]]
  };
  private static const THROWABLE_LABELS:Object={1:"Damage",3:"Armor penetration",132:"Stopping power",167:"Stamina damage",23:"Explosion radius",15:"Weight",168:"Fuse time",169:"Throw speed",170:"Defuse time"};
  private static const THROWABLE_UNITS:Object={132:"%",23:"m",15:"kg",168:"s",169:"m/s",170:"s"};
  private function throwable(name:String):Object {
   var Data:Class=domain.getDefinition("survarium.data.ItemData") as Class;
   var d:Object=new Data();d.name=name;d.category=THROWABLES[name][0];d.superCategory=domain.getDefinition("survarium.item.ItemFactory").getCategory(d.category);d.descriptor=100;d.modifiers=[];d.properties=[];
   for each(var p:Array in THROWABLES[name][1])d.properties.push({prop_id:p[0],prop_value:p[1]});
   return d;
  }
  // Each bar of a grenade's or a trap's card: "label=value", in the card's order (166, the pocket
  // units, is no bar).
  private function throwableLines(card:Object,d:Object):Array {
   var out:Array=[];
   for each(var p:Array in THROWABLES[d.name][1])if(THROWABLE_LABELS[p[0]])out.push(THROWABLE_LABELS[p[0]]+"="+nativeText(card,THROWABLE_LABELS[p[0]]));
   return out;
  }
  private function throwableChecks():void {
   Scene.isShift=false;
   var names:Array=["rgd5","smoke","trap"],released:Object={},name:String,d:Object,other:Object=throwable("rgd5");
   profile.setKitModifiers(kitFloats({}));
   for each(name in names){
    Scene.isAlt=false;d=throwable(name);shows(d,other);released[name]=throwableLines(cards[0],d).join();
    var want:Array=[];
    for each(var p:Array in THROWABLES[name][1])if(THROWABLE_LABELS[p[0]])want.push(THROWABLE_LABELS[p[0]]+"="+Number(p[1])+(THROWABLE_UNITS[p[0]]||""));
    require(released[name]==want.join(),"released "+name+": "+released[name]);
    for(var i:int=1;i<=9;i++){var row:Object=cards[0].itemCompare.getChildByName("diff_"+i);require(!row || !row.visible || row.indicator.label!="Pocket slots","no pocket units bar: "+name);}
    Scene.isAlt=true;shows(d,other);
    require(throwableLines(cards[0],d).join()==released[name],"Alt, no skills: "+name+" "+throwableLines(cards[0],d).join());
   }
   // The lines a rule changes, the others as released.
   var expect:Function=function(name:String,changes:Object):String{
    var lines:Array=released[name].split(",");
    for(var i:int=0;i<lines.length;i++){var label:String=lines[i].split("=")[0];if(changes[label])lines[i]=label+"="+changes[label];}
    return lines.join();
   };
   var cases:Array=[
    // Grenadier, ranks 1 and 2: throw_power x (1 + 0.1 a rank) (0x30A2DE).
    [{27:.1},{rgd5:{"Throw speed":"17 -> 18.7m/s"},smoke:{"Throw speed":"17 -> 18.7m/s"},trap:{}}],
    [{27:.2},{rgd5:{"Throw speed":"17 -> 20.4m/s"},smoke:{"Throw speed":"17 -> 20.4m/s"},trap:{}}],
    // Device damage and splash radius +10 %: the splash's damage, stopping power and stamina
    // damage, the trap's hit (0x30A350, 0x304A40); the radius (0x30A350). Penetration stays.
    [{11:.1,12:.1},{rgd5:{"Damage":"130 -> 143","Stopping power":"10 -> 11%","Stamina damage":"10 -> 11","Explosion radius":"10 -> 11m"},
                    smoke:{},trap:{"Damage":"60 -> 66"}}],
    // A sum below -1 stops at 0, as max(0, 1 + S).
    [{27:-2},{rgd5:{"Throw speed":"17 -> 0m/s"},smoke:{"Throw speed":"17 -> 0m/s"},trap:{}}]
   ];
   for each(var c:Array in cases){
    profile.setKitModifiers(kitFloats(c[0]));
    for each(name in names){
     d=throwable(name);Scene.isAlt=true;shows(d,other);
     var got:String=throwableLines(cards[0],d).join();
     require(got==expect(name,c[1][name]),"floats "+JSON.stringify(c[0])+" "+name+": "+got+" (want "+expect(name,c[1][name])+")");
     Scene.isAlt=false;shows(d,other);
     require(throwableLines(cards[0],d).join()==released[name],"released with floats: "+name);
    }
   }
   // Faster and more is green.
   profile.setKitModifiers(kitFloats({27:.2,11:.1}));Scene.isAlt=true;
   d=throwable("rgd5");shows(d,other);
   require(changeColours(nativeField(cards[0],"Throw speed")).join()==[NEUTRAL,NEUTRAL,GREEN].join() && changeColours(nativeField(cards[0],"Damage")).join()==[NEUTRAL,NEUTRAL,GREEN].join(),"rgd-5: faster and more is green");
   // An exe that sends no floats: the card's own numbers.
   profile.setKitModifiers(null);d=throwable("rgd5");shows(d,other);
   require(throwableLines(cards[0],d).join()==released.rgd5,"no floats: the grenade's own numbers");
   Scene.isAlt=false;
  }
  // Held Alt over a weapon whose own movement line is a penalty, with Bear Arms (kit float 62 above 0):
  // the line reads "-5% -> 0%" (weapon_core::initialize 0x2E2C5C..0x2E2C9C leaves a penalty out); a
  // bonus, Bear Arms off and an exe that sends no floats leave it as released.
  private function movingWeapon(pkp:Boolean,movement:String):Object {
   var d:Object=item(pkp);d.properties.push({prop_id:105,prop_value:movement});
   domain.getDefinition("survarium.item.WeaponMovement").expose(d);
   var amount:Number=Number(movement)*100;
   for each(var m:Object in d.modifiers)m.textValue="Movement speed: <"+(amount<0?"red":"green")+">"+(amount>0?"+":"")+amount+"%</"+(amount<0?"red":"green")+">\n";
   return d;
  }
  // The built-in lines of a card: [the rendered text of each, the label each was given].
  private function defaultLines(card:Object):Array {
   var list:Object=card.defaultModifiersList,shown:Array=[],labels:Array=[];
   for(var i:int=0;i<list.container.numChildren;i++)shown.push(list.container.getChildAt(i).textFieldLabel.text.replace(/\s+$/,""));
   for(i=0;i<list.dataProvider.length;i++)labels.push(list.dataProvider[i].label);
   return [shown.join("|"),labels.join("|")];
  }
  // A built-in line's colour at its last character.
  private function lastColour(card:Object,index:int):uint {
   var field:TextField=card.defaultModifiersList.container.getChildAt(index).textFieldLabel,t:String=field.text.replace(/\s+$/,"");
   return uint(field.getTextFormat(t.length-1,t.length).color)&0xFFFFFF;
  }
  private function firstValueColour(card:Object,index:int):uint {
   var field:TextField=card.defaultModifiersList.container.getChildAt(index).textFieldLabel,at:int=field.text.indexOf(": ")+2;
   return uint(field.getTextFormat(at,at+1).color)&0xFFFFFF;
  }
  private var heavy:Object;
  private function bearArmsChecks():void {
   // The kit's armour (altChecks') has no Bear Arms: float 62 alone says it.
   Scene.isShift=false;Scene.isCtrl=false;
   heavy=movingWeapon(true,"-0.05");
   var light:Object=movingWeapon(false,"0.03");
   // Another built-in line beside the movement (a golden weapon's silver): never changed.
   var silver:Object={state:0,isDefault:true,icon:33,textValue:"Silver: <green>+10%</green>\n",props:[{prop_id:31,prop_value:"0.1"}]};
   var both:Object=movingWeapon(true,"-0.05");both.modifiersDefault.push(silver);both.modifiers=both.modifiersDefault;
   var off:Object=kitFloats({}),on:Object=kitFloats({62:1});
   profile.setKitModifiers(on);
   Scene.isAlt=false;shows(heavy,light);
   var released:Array=defaultLines(cards[0]),releasedHeight:Number=cards[0].defaultModifiersList.container.height;
   require(released[0]=="Movement speed: -5%","released, Bear Arms on: the weapon's own line ("+released[0]+")");
   require(defaultLines(cards[1])[0]=="Movement speed: +3%","released bonus: "+defaultLines(cards[1])[0]);
   Scene.isAlt=true;shows(heavy,light);
   var held:Array=defaultLines(cards[0]);
   require(held[0]=="Movement speed: -5% -> 0%","Alt, Bear Arms on, a penalty: "+held[0]);
   require(held[1]=="Movement speed: -5% -> <green>0%</green>\n","the current part green, the original and the arrow neutral: "+held[1]);
   require(lastColour(cards[0],0)==GREEN && firstValueColour(cards[0],0)!=GREEN && firstValueColour(cards[0],0)!=RED,"rendered: 0% green, -5% neutral ("+lastColour(cards[0],0)+", "+firstValueColour(cards[0],0)+")");
   require(Math.abs(cards[0].defaultModifiersList.container.height-releasedHeight)<.5,"the line keeps its height");
   require(defaultLines(cards[1])[0]=="Movement speed: +3%" && defaultLines(cards[1])[1]==light.modifiers[0].textValue,"Alt, Bear Arms on, a bonus: unchanged ("+defaultLines(cards[1])[0]+")");
   Scene.isCtrl=true;shows(heavy,light);
   require(defaultLines(cards[0])[1]=="Movement speed: -5% -> 0%\n","Ctrl: the change neutral ("+defaultLines(cards[0])[1]+")");
   Scene.isCtrl=false;
   shows(both,light);
   require(defaultLines(cards[0])[0]=="Movement speed: -5% -> 0%|Silver: +10%" && defaultLines(cards[0])[1].split("|")[1]==silver.textValue,"another built-in line unchanged: "+defaultLines(cards[0])[0]);
   // Bear Arms off, or no floats: a penalty stays.
   profile.setKitModifiers(off);shows(heavy,light);
   require(defaultLines(cards[0])[1]==heavy.modifiers[0].textValue && defaultLines(cards[1])[1]==light.modifiers[0].textValue,"Alt, Bear Arms off: both lines as released ("+defaultLines(cards[0])[0]+")");
   profile.setKitModifiers(null);shows(heavy,light);
   require(defaultLines(cards[0])[1]==heavy.modifiers[0].textValue,"no floats: the penalty as released");
   // A weapon without a movement line: nothing to change.
   profile.setKitModifiers(on);
   var plain:Object=item(false);shows(plain,heavy);
   require(defaultLines(cards[0])[0]=="","a weapon without a movement line has no built-in line");
   Scene.isAlt=false;shows(heavy,light);
   require(defaultLines(cards[0])[1]==heavy.modifiers[0].textValue,"Alt released: the weapon's own line");
   // altChecks' skills again, for the open-card checks that follow.
   profile.setKitModifiers(kitFloats({49:-.1,13:.12,21:.12,22:.15,56:-.1}));
   Scene.isShift=true;shows(item(false),items[0]);
  }
  private function loaded(e:Event):void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;Scene=domain.getDefinition("MainScene");Scene.stage=this;Scene.scaleFactor=1;
    var css:StyleSheet=new StyleSheet();css.setStyle("red",{color:"#CF4242"});css.setStyle("white",{color:"#D7DEDC"});css.setStyle("gray",{color:"#8A9490"});Scene.textStyleSheet=css;
    Loc=domain.getDefinition("survarium.data.LocalizationData");Loc.setData(texts("english"));
    Details=domain.getDefinition("survarium.item.WeaponDetails");
    DetailsView=domain.getDefinition("survarium.item.ItemDetailsView") as Class;
    Caption=domain.getDefinition("survarium.item.WeaponDetailCaption") as Class;
    var Props:Class=domain.getDefinition("survarium.data.PropertiesDescriptor") as Class;
    var props:Object=new Props("properties"),list:Array=[];
    for each(var spec:Array in [[1,"Damage",1,""],[2,"Dispersion",0,""],[3,"Armor penetration",1,""],[4,"Rate of fire",1,""],[15,"Weight",0,"kg"],[24,"Distance",1,"m"],[132,"Stopping power",1,"%"],[133,"Aim time",0,"s"],[134,"Magazine",1,""]])list.push({prop_id:spec[0],prop_name:spec[1],direction:spec[2],postfix:spec[3],comparable:true,min_value:0,max_value:100});
    // A medicine card's lines, as static_game_parameters has them (13 has no unit there).
    for each(spec in [[13,"Intoxication",1,""],[17,"Pain protection",1,"%"],[18,"Duration",1,"s"],[19,"Activation",0,"s"],[135,"Healing: arms / legs",1,"%"],[136,"Healing: body / head",1,"%"],[137,"Pain relief",1,"%"]])list.push({prop_id:spec[0],prop_name:spec[1],direction:spec[2],postfix:spec[3],comparable:true,min_value:0,max_value:100});
    for(var id:int=138;id<=162;id++)list.push({prop_id:id,comparable:false});
    // A grenade's and a trap's (throwable-cards-1); 166 the pocket units (pocket-units-1), no bar.
    for each(spec in [[23,"Explosion radius",1,"m"],[167,"Stamina damage",1,""],[168,"Fuse time",0,"s"],[169,"Throw speed",1,"m/s"],[170,"Defuse time",1,"s"]])list.push({prop_id:spec[0],prop_name:spec[1],direction:spec[2],postfix:spec[3],comparable:true,min_value:0,max_value:100});
    list.push({prop_id:166,prop_name:"Pocket slots",comparable:false});
    // A round's lines (AmmoDetails labels them by these names), and Pierces (pierce-line-1), no bar.
    for each(spec in [[21,"Damage"],[22,"Armor penetration"],[163,"Bullet speed"],[16,"Rounds in a pack"],[174,"Pierces"]])list.push({prop_id:spec[0],prop_name:spec[1],comparable:false});
    // The rolls' own properties, multiplied (modifier_type 1) as the exe tells the movie (mod_type 2).
    for each(id in [102,115,119])list.push({prop_id:id,comparable:false,modifier_type:1});
    // A weapon's own movement line (weapon-movement-ui-1), higher is better.
    list.push({prop_id:105,prop_name:"Movement speed",direction:1,comparable:false,modifier_type:1});
    props.setPropertiesDictionary(list);domain.getDefinition("survarium.controls.ItemCompare").setPropsDescriptor(props);
    var Container:Class=domain.getDefinition("survarium.data.IconContainer") as Class;
    var Atlas:Class=domain.getDefinition("LobbyMenu_ModifierIcons") as Class;
    new Container("modifiers",38,38,Bitmap(new Atlas()).bitmapData,true,0,0,0,false);
    items=[item(true),item(false)];
    var Descriptor:Class=domain.getDefinition("survarium.data.ItemDescriptor") as Class;
    var descriptor:Object=new Descriptor("InventoryItems");descriptor.setItemsDictionary([{dictId:3,item_properties:items[1].properties}].concat(roundDescriptors()));
    domain.getDefinition("survarium.item.ItemFactory").setItemsCompatibility(ROUND_PAIRS);
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
       // A round under a stat (held Shift: "• 7N22") steps its label in by the view's indent.
       var stepped:Number=name=="textFieldLabel" && row[name].text.charAt(0)=="•"?
        row[name].parent.localToGlobal(new Point(Object(DetailsView).SUBROW_INDENT,0)).x-row[name].parent.localToGlobal(new Point(0,0)).x:0;
       require(Math.abs(edge(row[name])-edge(original[name])-stepped)<.2,"original column alignment: "+name+" '"+row[name].text+"' "+[edge(row[name]),edge(original[name]),stepped,row[name].x,original[name].x]);
       require(Math.abs(row[name].transform.concatenatedMatrix.a-original[name].transform.concatenatedMatrix.a)<.001 && Math.abs(row[name].transform.concatenatedMatrix.d-original[name].transform.concatenatedMatrix.d)<.001,"original text scale: "+name);
      }
      require(row.textFieldLabel.textColor==original.textFieldLabel.textColor,"original label color");
      require([15911311,8890390,9834254].indexOf(row.textFieldValue.textColor)>=0,"original stat palette");
      require(row.textFieldValue.textWidth<row.textFieldValue.width,"value fits row: "+row.textFieldValue.text);
      // The native stride, or whole strides more where the compared card's row wraps.
      if(j+1<child.numChildren && !(child.getChildAt(j+1) is Caption) && row.textFieldLabel.numLines==1){
       var stride:Number=card.itemCompare.diff_2.y-card.itemCompare.diff_1.y,step:Number=(child.getChildAt(j+1).y-child.getChildAt(j).y)/stride;
       // A round under a stat ("• 7N22", held Shift) takes SUBROW_STRIDE of a row.
       if(row.textFieldLabel.text.charAt(0)=="•")step/=Object(DetailsView).SUBROW_STRIDE;
       require(step>=1-.01 && Math.abs(step-Math.round(step))<.01,"original row spacing: "+row.textFieldLabel.text+" "+step);
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
    // A row without a value (a stat's header over its rounds, held Shift): the label has the row.
    else if(!value.text.length)valueStart+=value.width+6;
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
    if(frame==20 && loaderInfo.parameters.pierce){pierceCapture(loaderInfo.parameters.pierce);return;}
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
     tacticalChecks();
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
     pierceChecks();
     altChecks();
     throwableChecks();
     medicineChecks();
     bearArmsChecks();
    }
    if(frame==140){Scene.isAlt=true;}
    if(frame==150){require(line(cards[0],"Reload")=="Reload: 3 -> 2.46s" && nativeText(cards[0],"Aim time")=="0.25 -> 0.21s","Alt pressed over an open card");Scene.isAlt=false;}
    if(frame==160){require(text(cards[0])==released && nativeText(cards[0],"Aim time")=="0.25s","Alt let go over an open card");profile.setKitModifiers(medicalKit(3,2,3));shows(medicine("painkiller"),medicine("medkit"));}
    if(frame==170){require(nativeText(cards[0],"Pain protection")=="10%","an open painkiller card, Alt up");Scene.isAlt=true;}
    if(frame==180){require(nativeText(cards[0],"Pain protection")=="10 -> 13%" && nativeText(cards[0],"Duration")=="10 -> 14.5s","Alt pressed over an open medicine card");Scene.isAlt=false;}
    if(frame==190){require(nativeText(cards[0],"Pain protection")=="10%" && nativeText(cards[0],"Duration")=="10s","Alt let go over an open medicine card");profile.setKitModifiers(kitFloats({27:.2}));shows(throwable("rgd5"),throwable("smoke"));}
    if(frame==200){require(nativeText(cards[0],"Throw speed")=="17m/s","an open grenade card, Alt up");Scene.isAlt=true;}
    if(frame==210){require(nativeText(cards[0],"Throw speed")=="17 -> 20.4m/s","Alt pressed over an open grenade card");Scene.isAlt=false;}
    if(frame==220){require(nativeText(cards[0],"Throw speed")=="17m/s","Alt let go over an open grenade card");profile.setKitModifiers(kitFloats({62:1}));shows(heavy,items[1]);}
    if(frame==230){require(defaultLines(cards[0])[0]=="Movement speed: -5%","an open card with Bear Arms, Alt up");Scene.isAlt=true;}
    if(frame==240){require(defaultLines(cards[0])[0]=="Movement speed: -5% -> 0%" && lastColour(cards[0],0)==GREEN,"Alt pressed over an open card with Bear Arms: "+defaultLines(cards[0])[0]);Scene.isAlt=false;}
    if(frame==250){require(defaultLines(cards[0])[0]=="Movement speed: -5%","Alt let go over an open card with Bear Arms");shows(items[0],items[1]);finish(null);}
   }catch(err:Error){finish("frame "+frame+": "+err+" "+err.getStackTrace());}
  }
  private function finish(error:String):void {removeEventListener(Event.ENTER_FRAME,tick);if(!error){Scene.isCtrl=loaderInfo.parameters.neutral=="1";if(loaderInfo.parameters.compact=="1")Scene.isShift=false;Scene.isAlt=loaderInfo.parameters.alt=="1";
   // For a capture: a kit whose gloves reload faster but aim slower (a green and a red change).
   if(loaderInfo.parameters.mixed=="1"){wearing([piece(5,[[114,"0.10"],[102,"0.20"]]),piece(2,[[115,"-0.04"]])]);shows(item(false),items[0]);}}trace("DETAILS RESULT "+(error?"fail: "+error:"pass"));}
 }
}
