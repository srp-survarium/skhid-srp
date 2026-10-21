package
{
   import flash.display.MovieClip;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.TeamSide;
   
   public dynamic class SrvPlayerList extends MovieClip
   {
      
      public var minimap:MineMinimapTab;
      
      public var players:PlayerListz;
      
      public var _players:Array;
      
      public var playerUpdate:Object;
      
      public function SrvPlayerList()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function addEv(e:ButtonEvent) : void
      {
      }
      
      internal function frame1() : *
      {
         _players = [{
            id:1,
            team:TeamSide.ENEMY,
            name:"1",
            ping:50
         },{
            id:2,
            team:TeamSide.ALLY,
            name:"2",
            ping:18
         },{
            id:3,
            team:TeamSide.ENEMY,
            name:"3",
            ping:2000
         },{
            id:4,
            team:TeamSide.ALLY,
            name:"4",
            ping:18
         },{
            id:5,
            team:TeamSide.ENEMY,
            name:"5",
            ping:2000
         },{
            id:6,
            team:TeamSide.ALLY,
            name:"6",
            ping:18
         },{
            id:7,
            team:TeamSide.ENEMY,
            name:"7",
            ping:2000
         },{
            id:8,
            team:TeamSide.ALLY,
            name:"8",
            ping:18
         },{
            id:9,
            team:TeamSide.ENEMY,
            name:"9",
            ping:2000
         },{
            id:10,
            team:TeamSide.ALLY,
            name:"10",
            ping:18
         },{
            id:11,
            team:TeamSide.ENEMY,
            name:"11",
            ping:2000
         },{
            id:12,
            team:TeamSide.ALLY,
            name:"12",
            ping:18
         },{
            id:13,
            team:TeamSide.ENEMY,
            name:"13",
            ping:2000
         },{
            id:14,
            team:TeamSide.ALLY,
            name:"14",
            ping:18
         },{
            id:15,
            team:TeamSide.ENEMY,
            name:"15",
            ping:2000
         },{
            id:16,
            team:TeamSide.ALLY,
            name:"16",
            ping:18
         },{
            id:17,
            team:TeamSide.ENEMY,
            name:"17",
            ping:2000
         },{
            id:18,
            team:TeamSide.ALLY,
            name:"18",
            ping:18
         },{
            id:19,
            team:TeamSide.ENEMY,
            name:"19",
            ping:2000
         },{
            id:20,
            team:TeamSide.ALLY,
            name:"20",
            ping:18
         },{
            id:21,
            team:TeamSide.ENEMY,
            name:"21",
            ping:2000
         },{
            id:22,
            team:TeamSide.ALLY,
            name:"22",
            ping:116
         },{
            id:23,
            team:TeamSide.ENEMY,
            name:"23",
            ping:2000
         },{
            id:24,
            team:TeamSide.ALLY,
            name:"24",
            ping:18
         },{
            id:25,
            team:TeamSide.ENEMY,
            name:"25",
            ping:2000
         },{
            id:26,
            team:TeamSide.ALLY,
            name:"26",
            ping:18
         },{
            id:27,
            team:TeamSide.ENEMY,
            name:"27",
            ping:2000
         },{
            id:28,
            team:TeamSide.ALLY,
            name:"28",
            ping:116
         },{
            id:29,
            team:TeamSide.ENEMY,
            name:"29",
            ping:2000
         },{
            id:30,
            team:TeamSide.ALLY,
            name:"30",
            ping:116
         }];
         playerUpdate = {
            id:14,
            kills:5,
            deaths:2,
            ping:23,
            rating:600
         };
         stop();
      }
   }
}

